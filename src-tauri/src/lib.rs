use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use window_vibrancy::apply_mica;
use std::path::Path;
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(handle_shortcut)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![list_stickers])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            let _ = apply_mica(&window, None);

            // Shortcut: Alt + .
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Period);
            app.global_shortcut().register(shortcut).expect("Failed to register global shortcut");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// open the stickerboard
fn handle_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        if shortcut.matches(Modifiers::ALT, Code::Period) {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    }
}

// list stickers from specified dir
// The Data Model sent to Frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sticker {
    name: String,
    path: String,
    format: String,
}

// The Command
#[tauri::command]
fn list_stickers() -> Vec<Sticker> {
    // 1. Get User Home Directory (Simple way for Windows)
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let sticker_path = Path::new(&user_profile).join("Pictures\\Stickers");

    let mut stickers = Vec::new();

    // 2. Check if folder exists
    if !sticker_path.exists() {
        return stickers; // Return empty if no folder
    }

    // 3. Walk directory recursively
    for entry in WalkDir::new(sticker_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // Filter for images only
        if path.is_file() {
            if let Some(extension) = path.extension() {
                let ext_str = extension.to_string_lossy().to_lowercase();
                if ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext_str.as_str()) {
                    
                    stickers.push(Sticker {
                        name: path.file_stem().unwrap().to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        format: ext_str,
                    });
                }
            }
        }
    }
    
    stickers
}