use crate::models::AppSettings;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use window_vibrancy::{apply_acrylic, apply_mica};

pub fn get_app_dir(app_handle: &AppHandle) -> PathBuf {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    if !app_dir.exists() { let _ = fs::create_dir_all(&app_dir); }
    app_dir
}

pub fn load_settings(app_handle: &AppHandle) -> AppSettings {
    let app_dir = get_app_dir(app_handle);
    let settings_path = app_dir.join("settings.json");
    
    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppSettings::default()
    }
}

pub fn save_settings(app_handle: &AppHandle, settings: &AppSettings) {
    let app_dir = get_app_dir(app_handle);
    let settings_path = app_dir.join("settings.json");
    let _ = fs::write(settings_path, serde_json::to_string(settings).unwrap());
    
    #[cfg(target_os = "windows")]
    if let Some(window) = app_handle.get_webview_window("main") {
        apply_theme_to_window(&window, &settings.theme);
    }
}

pub fn resolve_sticker_path(app_handle: &AppHandle) -> PathBuf {
    let settings = load_settings(app_handle);
    
    if !settings.sticker_path.is_empty() {
        let path = PathBuf::from(&settings.sticker_path);
        if path.exists() {
            return path;
        }
    }

    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    Path::new(&user_profile).join("Pictures\\Stickers")
}

#[cfg(target_os = "windows")]
pub fn apply_theme_to_window(window: &tauri::WebviewWindow, theme: &str) {
    if theme == "mica" {
        let _ = apply_mica(window, None);
    } else {
        let _ = apply_acrylic(window, Some((0, 0, 0, 10))); 
    }
}

pub fn is_animated_webp(path: &Path) -> bool {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    
    let mut header = [0u8; 12];
    if file.read_exact(&mut header).is_err() { return false; }
    
    // Check if it's actually a PNG (APNG) mislabeled as WEBP
    if &header[0..8] == b"\x89PNG\r\n\x1a\n" {
        // Read the next ~256 bytes to look for the "acTL" animation control chunk
        let mut png_buffer = [0u8; 256];
        let bytes_read = file.read(&mut png_buffer).unwrap_or(0);
        return png_buffer[..bytes_read].windows(4).any(|w| w == b"acTL");
    }
    
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WEBP" { 
        return false; 
    }
    
    loop {
        let mut chunk_header = [0u8; 8];
        if file.read_exact(&mut chunk_header).is_err() { break; }
        
        if &chunk_header[0..4] == b"VP8X" {
            let mut vp8x_data = [0u8; 10];
            if file.read_exact(&mut vp8x_data).is_ok() {
                let has_animation_bit = (vp8x_data[0] & 0x02) != 0;
                if has_animation_bit {
                    return true;
                }
            }
            let _ = file.seek(SeekFrom::Current(-10));
        }

        if &chunk_header[0..4] == b"ANIM" {
            return true;
        }
        
        let chunk_size = u32::from_le_bytes(chunk_header[4..8].try_into().unwrap()) as u64;
        let padded_size = chunk_size + (chunk_size & 1);
        
        if file.seek(SeekFrom::Current(padded_size as i64)).is_err() { break; }
    }
    
    false
}