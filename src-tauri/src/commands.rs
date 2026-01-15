use crate::models::{Sticker, AppSettings, AppState};
use crate::{database, clipboard, utils, library};
use tauri::{AppHandle, Manager};
use rusqlite::Connection;
use chrono::Utc;
use std::thread;
use std::time::Duration;
use std::fs;

// Helper to get connection
fn get_conn(app: &AppHandle) -> Connection {
    let state = app.state::<AppState>();
    Connection::open(&state.db_path).unwrap()
}

#[tauri::command]
pub async fn search_stickers(app: AppHandle, query: String, tab: String, limit: usize) -> Result<Vec<Sticker>, String> {
    let conn = get_conn(&app);
    database::search_stickers(&conn, query, tab, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn select_sticker(app: AppHandle, path: String) -> Result<(), String> {
    let conn = get_conn(&app);
    let now = Utc::now().timestamp();
    let _ = database::update_usage(&conn, &path, now);

    let clipboard_backup = clipboard::backup();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    let app_dir = utils::get_app_dir(&app);
    let thumb_dir = app_dir.join("thumbnails");

    clipboard::copy_sticker_to_clipboard(&path, &thumb_dir)?;

    thread::sleep(Duration::from_millis(150));
    clipboard::send_paste_event()?;

    // restore user clipboard
    thread::sleep(Duration::from_millis(600));
    if let Some(backup) = clipboard_backup {
        clipboard::restore(backup);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_packs(app: AppHandle) -> Result<Vec<String>, String> {
    let conn = get_conn(&app);
    database::get_packs(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_favorite(app: AppHandle, path: String) -> bool {
    let conn = get_conn(&app);
    database::toggle_favorite(&conn, &path).unwrap_or(false)
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    utils::load_settings(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) {
    utils::save_settings(&app, &settings);
}

#[tauri::command]
pub fn apply_theme(app: AppHandle, theme: String) {
    #[cfg(target_os = "windows")]
    if let Some(window) = app.get_webview_window("main") {
        utils::apply_theme_to_window(&window, &theme);
    }
}

#[tauri::command]
pub fn wipe_data(app: AppHandle, data_type: String) -> bool {
    let conn = get_conn(&app);
    let res = match data_type.as_str() {
        "history" => database::wipe_history(&conn),
        "favorites" => database::wipe_favorites(&conn),
        "db" => {
            let _ = database::reset_library(&conn);
            // also clear thumbs
            let app_dir = utils::get_app_dir(&app);
            let thumb_dir = app_dir.join("thumbnails");
            if thumb_dir.exists() {
                let _ = fs::remove_dir_all(&thumb_dir);
                let _ = fs::create_dir_all(&thumb_dir);
            }
            Ok(())
        },
        _ => Ok(())
    };
    res.is_ok()
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub async fn refresh_library(app: AppHandle) {
    let app_dir = utils::get_app_dir(&app);
    let db_path = app_dir.join("library.db");
    let thumb_dir = app_dir.join("thumbnails");
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        library::index_library(&handle, db_path, thumb_dir);
    });
}