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

// reads 21 bytes to find vp8x flag
fn is_animated_webp(path: &Path) -> bool {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    
    let mut buffer = [0; 256];
    let bytes_read = file.read(&mut buffer).unwrap_or(0);
    
    if bytes_read < 21 { 
        return false; 
    }
    
    if &buffer[0..4] != b"RIFF" || &buffer[8..12] != b"WEBP" {
        return false;
    }
    
    if &buffer[12..16] == b"VP8X" {
        let has_animation_bit = (buffer[20] & 0x02) != 0;
        
        if has_animation_bit {
            return buffer[..bytes_read].windows(4).any(|window| window == b"ANIM");
        }
    }
    
    false
}

pub fn copy_sticker_to_clipboard(path: &str, thumb_dir: &Path) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    let extension = path_buf.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    // if its an animated format we treat them differently
    // might add more formats in future if required
    let is_animated = 
    if extension == "gif"{
        true
    } else if extension == "webp"{
        is_animated_webp(&path_buf)
    } else {
        false
    };

    let hash = {
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(path.as_bytes());
        hex::encode(hasher.finalize())
    };

    if is_animated {
        let _ = (|| -> Result<(), String> {
            let mut load_path = path.to_string();
            if extension == "gif" {
                let thumb_path_gif = thumb_dir.join(format!("{}.gif", hash));
                if thumb_path_gif.exists() {
                    load_path = thumb_path_gif.to_string_lossy().to_string();
                }
            }
            let _clip = WinClipboard::new_attempts(10).map_err(|e| e.to_string())?;
            let files = vec![load_path];
            formats::FileList.write_clipboard(&files).map_err(|e| e.to_string())?;
            Ok(())
        })().map_err(|e| format!("Clipboard error: {}", e))?;
    } else {
        let thumb_path = thumb_dir.join(format!("{}.webp", hash));

        let load_path = if thumb_path.exists() { thumb_path } else { path_buf };

        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        
        let img = ImageReader::open(&load_path)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .with_guessed_format()
            .map_err(|e| format!("Failed to guess format: {}", e))?
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