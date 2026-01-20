use crate::models::{Sticker, AppSettings, AppState};
use crate::{database, clipboard, utils, library};
use tauri::{AppHandle, Manager};
use rusqlite::Connection;
use chrono::Utc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::Path;
use sha2::{Sha256, Digest};

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
pub fn delete_sticker(app: AppHandle, path: String) -> Result<(), String> {
    let conn = get_conn(&app);

    database::delete_sticker(&conn, &path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn rename_sticker(app: AppHandle, path: String, new_name: String) -> Result<(), String> {
    let conn = get_conn(&app);
    let old_path_obj = Path::new(&path);
    
    if !old_path_obj.exists() {
        return Err("File not found".to_string());
    }

    let parent = old_path_obj.parent().ok_or("Invalid path")?;
    let extension = old_path_obj.extension().ok_or("No extension")?;
    let new_filename = format!("{}.{}", new_name, extension.to_string_lossy());
    let new_path_obj = parent.join(new_filename);
    
    if new_path_obj.exists() {
        return Err("A file with that name already exists".to_string());
    }

    fs::rename(old_path_obj, &new_path_obj).map_err(|e| e.to_string())?;

    let new_path_str = new_path_obj.to_string_lossy().to_string();
    database::rename_sticker(&conn, &path, &new_path_str, &new_name).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn move_sticker(app: AppHandle, path: String, pack_name: String) -> Result<(), String> {
    let conn = get_conn(&app);
    let old_path_obj = Path::new(&path);

    if !old_path_obj.exists() {
        return Err("Source file not found".to_string());
    }

    let file_name = old_path_obj.file_name().ok_or("Invalid filename")?;
    
    let root_dir = utils::resolve_sticker_path(&app);
    if pack_name.contains("..") {
        return Err("Invalid pack name: Traversal characters (..) are not allowed".to_string());
    }

    let target_pack_dir = match database::get_pack_path(&conn, &pack_name).unwrap_or(None) {
        Some(existing_sticker_path) => {
            let p = Path::new(&existing_sticker_path);
            p.parent().unwrap_or(&root_dir.join(&pack_name)).to_path_buf()
        },
        None => root_dir.join(&pack_name)
    };
    if !target_pack_dir.starts_with(&root_dir) {
        return Err("Security Violation: Cannot move sticker outside of library directory.".to_string());
    }
    if !target_pack_dir.exists() {
        fs::create_dir_all(&target_pack_dir).map_err(|e| e.to_string())?;
    }

    let new_path_obj = target_pack_dir.join(file_name);
    
    if new_path_obj.exists() {
        return Err("A file with that name already exists in the destination".to_string());
    }

    fs::rename(old_path_obj, &new_path_obj).map_err(|e| e.to_string())?;

    let app_dir = utils::get_app_dir(&app);
    let thumb_dir = app_dir.join("thumbnails");
    
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    let old_hash = hex::encode(hasher.finalize());
    let old_thumb_path = thumb_dir.join(format!("{}.webp", old_hash));

    let mut hasher = Sha256::new();
    hasher.update(new_path_obj.to_string_lossy().as_bytes());
    let new_hash = hex::encode(hasher.finalize());
    let new_thumb_path = thumb_dir.join(format!("{}.webp", new_hash));

    let new_db_thumb_path_str: String;

    if old_thumb_path.exists() {
        fs::rename(&old_thumb_path, &new_thumb_path).map_err(|e| e.to_string())?;
        new_db_thumb_path_str = new_thumb_path.to_string_lossy().to_string();
    } else {
        new_db_thumb_path_str = new_path_obj.to_string_lossy().to_string();
    }

    let new_path_str = new_path_obj.to_string_lossy().to_string();
    
    database::move_sticker(&conn, &path, &new_path_str, &pack_name, &new_db_thumb_path_str)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn update_sticker(app: AppHandle, path: String, new_name: Option<String>, new_pack: Option<String>, is_favorite: Option<bool>) -> Result<(), String> {
    let conn = get_conn(&app);
    let current_path = Path::new(&path);
    
    if !current_path.exists() {
        return Err("Sticker file not found on disk.".to_string());
    }

    let root_dir = utils::resolve_sticker_path(&app);
    let mut final_path_buf = current_path.to_path_buf();
    
    if let Some(pack) = &new_pack {
        let target_pack_dir = root_dir.join(pack);
        if !target_pack_dir.exists() {
            fs::create_dir_all(&target_pack_dir).map_err(|e| e.to_string())?;
        }
        final_path_buf = target_pack_dir.join(final_path_buf.file_name().unwrap());
    }

    if let Some(name) = &new_name {
        let ext = final_path_buf.extension().unwrap_or_default().to_string_lossy();
        let safe_name: String = name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        final_path_buf.set_file_name(format!("{}.{}", safe_name, ext));
    }

    let final_path_str = final_path_buf.to_string_lossy().to_string();

    if final_path_str != path {
        if final_path_buf.exists() {
            return Err("A sticker with this name/pack already exists.".to_string());
        }

        fs::rename(&path, &final_path_buf).map_err(|e| format!("FS Error: {}", e))?;

        let mut query = "UPDATE stickers SET path = ?1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(final_path_str.clone())];
        
        let mut param_idx = 2;
        
        if let Some(name) = &new_name {
            query.push_str(&format!(", name = ?{}", param_idx));
            params.push(Box::new(name.clone()));
            param_idx += 1;
        }
        
        if let Some(pack) = &new_pack {
            query.push_str(&format!(", pack = ?{}", param_idx));
            params.push(Box::new(pack.clone()));
            param_idx += 1;
        }

        query.push_str(&format!(" WHERE path = ?{}", param_idx));
        params.push(Box::new(path.clone()));
        
        if let Some(name) = &new_name {
            database::rename_sticker(&conn, &path, &final_path_str, name).map_err(|e| e.to_string())?;
        } else {
            let old_name: String = conn.query_row("SELECT name FROM stickers WHERE path = ?1", [&path], |r| r.get(0)).unwrap_or_default();
            database::rename_sticker(&conn, &path, &final_path_str, &old_name).map_err(|e| e.to_string())?;
        }

        if let Some(pack) = &new_pack {
            // update pack column directly
            conn.execute("UPDATE stickers SET pack = ?1 WHERE path = ?2", [pack, &final_path_str]).map_err(|e| e.to_string())?;
        }
    }

    if let Some(fav) = is_favorite {
        let target = if final_path_str != path { &final_path_str } else { &path };
        let val = if fav { 1 } else { 0 };
        conn.execute("UPDATE stickers SET is_favorite = ?1 WHERE path = ?2", rusqlite::params![val, target])
            .map_err(|e| e.to_string())?;
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