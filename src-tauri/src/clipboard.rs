use arboard::{Clipboard, ImageData};
use clipboard_win::{Clipboard as WinClipboard, raw, Getter, Setter, formats};
use enigo::{Enigo, Key, Keyboard, Settings, Direction};
use image::ImageReader;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub struct ClipboardBackup {
    data: Vec<(u32, Vec<u8>)>,
}

pub fn backup() -> Option<ClipboardBackup> {
    let _clip = WinClipboard::new_attempts(10).ok()?;
    let available_formats: Vec<u32> = raw::EnumFormats::new().collect();
    let mut backup_data = Vec::new();
    
    // read data per format
    for format_id in available_formats {
        // skip GDI formats as they cant be saved as raw bytes
        // check ref for formats
        if [2, 3, 9, 14, 17].contains(&format_id) { continue; }

        let mut buffer = Vec::new();
        if formats::RawData(format_id).read_clipboard(&mut buffer).is_ok() && !buffer.is_empty() {
            backup_data.push((format_id, buffer));
        }
    }

    if backup_data.is_empty() {
        None
    } else {
        Some(ClipboardBackup { data: backup_data })
    }
}

pub fn restore(backup: ClipboardBackup) {
    if let Ok(_clip) = WinClipboard::new_attempts(20) {
        let _ = raw::empty();
        for (format_id, bytes) in backup.data {
            let _ = formats::RawData(format_id).write_clipboard(&bytes);
        }
    }
}

pub fn copy_sticker_to_clipboard(path: &str, _thumb_dir: &Path) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    let extension = path_buf.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if ["gif", "webp"].contains(&extension.as_str()) {
        let _ = (|| -> Result<(), String> {
            let _clip = WinClipboard::new_attempts(10).map_err(|e| e.to_string())?;
            let files = vec![path.to_string()];
            formats::FileList.write_clipboard(&files).map_err(|e| e.to_string())?;
            Ok(())
        })().map_err(|e| format!("Clipboard error: {}", e))?;
    } else {
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        
        let img = ImageReader::open(&path_buf)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;
            
        let rgba = img.into_rgba8(); 
        let image_data = ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: Cow::from(rgba.into_raw()),
        };
        clipboard.set_image(image_data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn send_paste_event() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(Key::Control, Direction::Release);
    Ok(())
}