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