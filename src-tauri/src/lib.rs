use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use window_vibrancy::{apply_acrylic, apply_mica};
use std::path::Path;
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};
use arboard::{Clipboard, ImageData};
use image::ImageReader;
use image::EncodableLayout;
use std::borrow::Cow;
use std::thread;
use std::time::Duration;
use enigo::{Enigo, Key, Keyboard, Settings, Direction};
use clipboard_win::{formats, Clipboard as WinClipboard, Setter};

// sticker data model
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sticker {
    name: String,
    path: String,
    format: String,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(handle_shortcut)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![list_stickers, select_sticker])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            let _ = apply_acrylic(&window, Some((0,0,0,10)));

            // TODO: change Alt + . to user specified shortcut
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
#[tauri::command]
fn list_stickers() -> Vec<Sticker> {
    // TODO: Change this to user specified dir
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let sticker_path = Path::new(&user_profile).join("Pictures\\Stickers");

    let mut stickers = Vec::new();

    if !sticker_path.exists() {
        return stickers; 
    }

    // recursively search for stickers in the main path
    for entry in WalkDir::new(sticker_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // filters for support image types
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

// places the sticker in your textbox
#[tauri::command]
async fn select_sticker(app: AppHandle, path: String) -> Result<(), String> {
    let path_buf = std::path::PathBuf::from(&path);
    let extension = path_buf.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // hide window, since we selected the sticker
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }

    if extension == "gif" {
        // pastes gif as file
        let _ = (|| -> Result<(), String> {
            let _clip = WinClipboard::new_attempts(10).map_err(|e| e.to_string())?;
            let files = vec![path.clone()];
            formats::FileList.write_clipboard(&files).map_err(|e| e.to_string())?;
            Ok(())
        })().map_err(|e| format!("Clipboard error: {}", e))?;

    } else {
        // for images
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        let img = ImageReader::open(&path).map_err(|e| e.to_string())?
            .decode().map_err(|e| e.to_string())?;
        
        let rgba = img.into_rgba8(); 
        let image_data = ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: Cow::from(rgba.into_raw()),
        };
        clipboard.set_image(image_data).map_err(|e| e.to_string())?;
    }

    // sleep to focus, need to optimize this
    thread::sleep(Duration::from_millis(150));

    // paste the sticker
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;

    Ok(())
}