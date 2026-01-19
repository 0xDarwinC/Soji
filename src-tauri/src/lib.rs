pub mod models;
pub mod database;
pub mod clipboard;
pub mod library;
pub mod utils;
pub mod commands;

use crate::models::AppState;
use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Manager, Emitter};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(handle_shortcut)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            let handle = app.handle().clone();
            
            let app_dir = utils::get_app_dir(&handle);
            let db_path = app_dir.join("library.db");
            let thumb_dir = app_dir.join("thumbnails");
            if !thumb_dir.exists() { let _ = fs::create_dir_all(&thumb_dir); }

            database::init_db(&db_path).expect("Failed to init DB");
            app.manage(AppState { 
                db_path: db_path.clone(),
                is_indexing: Arc::new(AtomicBool::new(false)) 
            });

            let settings = utils::load_settings(&handle);
            #[cfg(target_os = "windows")]
            utils::apply_theme_to_window(&window, &settings.theme);

            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Period);
            handle.global_shortcut().register(shortcut).expect("Failed to register global shortcut");
            
            tauri::async_runtime::spawn(async move {
                library::index_library(&handle, db_path, thumb_dir);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_stickers, 
            commands::select_sticker, 
            commands::toggle_favorite, 
            commands::hide_window,
            commands::get_settings,
            commands::save_settings,
            commands::wipe_data,
            commands::apply_theme,
            commands::refresh_library,
            commands::get_packs,
            commands::delete_sticker,
            commands::rename_sticker,
            commands::move_sticker,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_shortcut(app: &tauri::AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        if shortcut.matches(Modifiers::ALT, Code::Period) {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("app_shown", ());
                }
            }
        }
    }
}