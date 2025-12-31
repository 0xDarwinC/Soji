use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use window_vibrancy::{apply_acrylic, apply_mica};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};
use arboard::{Clipboard, ImageData};
use image::ImageReader;
use std::borrow::Cow;
use std::thread;
use std::time::Duration;
use enigo::{Enigo, Key, Keyboard, Settings, Direction};
use clipboard_win::{formats, Clipboard as WinClipboard, Setter};
use std::fs;
use chrono::Utc;
use rusqlite::{Connection, ToSql};
use sha2::{Sha256, Digest};
use std::io::Cursor;
use fast_image_resize as fr;
use fr::{Resizer, ResizeOptions, ResizeAlg, FilterType, PixelType};
use fr::images::Image;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::Instant;


// data models
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sticker {
    id: i64,
    name: String,
    path: String,
    thumbnail_path: String,
    format: String,
    pack: String,
    is_favorite: bool,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppSettings {
    sticker_path: String,
    recents_limit: usize,
    theme: String,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sticker_path: "".to_string(),
            recents_limit: 18,
            theme: "acrylic".to_string(),
        }
    }
}

struct AppState {
    db_path: PathBuf,
    is_indexing: Arc<AtomicBool>,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    current: usize,
    total: usize,
    eta_seconds: Option<u64>,
}

struct IndexingGuard(Arc<AtomicBool>);
impl Drop for IndexingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            let handle = app.handle().clone();
            
            let app_dir = get_app_dir(&handle);
            let db_path = app_dir.join("library.db");
            let thumb_dir = app_dir.join("thumbnails");
            if !thumb_dir.exists() { let _ = fs::create_dir_all(&thumb_dir); }

            init_db(&db_path).expect("Failed to init DB");
            app.manage(AppState { 
                db_path: db_path.clone(),
                is_indexing: Arc::new(AtomicBool::new(false)) 
            });

            let settings = load_settings_internal(&handle);
            #[cfg(target_os = "windows")]
            apply_theme_internal(&window, &settings.theme);

            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Period);
            handle.global_shortcut().register(shortcut).expect("Failed to register global shortcut");
            
            tauri::async_runtime::spawn(async move {
                index_library(&handle, db_path, thumb_dir);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_stickers, 
            select_sticker, 
            toggle_favorite, 
            hide_window,
            get_settings,
            save_settings,
            wipe_data,
            apply_theme,
            refresh_library,
            get_packs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_db(path: &Path) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS stickers (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            pack TEXT NOT NULL,
            format TEXT NOT NULL,
            thumbnail_path TEXT,
            width INTEGER,
            height INTEGER,
            is_favorite INTEGER DEFAULT 0,
            last_used INTEGER DEFAULT 0,
            use_count INTEGER DEFAULT 0
        )",
        [],
    )?;
    Ok(())
}

fn get_db_conn(app: &AppHandle) -> Connection {
    let state = app.state::<AppState>();
    Connection::open(&state.db_path).unwrap()
}

#[tauri::command]
async fn refresh_library(app: AppHandle) {
    let app_dir = get_app_dir(&app);
    let db_path = app_dir.join("library.db");
    let thumb_dir = app_dir.join("thumbnails");
    tauri::async_runtime::spawn(async move {
        index_library(&app, db_path, thumb_dir);
    });
}

// fills db with thumbnails
fn index_library(app: &AppHandle, db_path: PathBuf, thumb_dir: PathBuf) {
    let state = app.state::<AppState>();
    if state.is_indexing.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    let _guard = IndexingGuard(state.is_indexing.clone());

    let sticker_root = resolve_sticker_path(app);
    let mut conn = Connection::open(&db_path).unwrap();
    
    let mut existing_paths = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT path FROM stickers").unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
        for path in rows {
            if let Ok(p) = path { existing_paths.insert(p); }
        }
    }

    let walker = WalkDir::new(sticker_root).into_iter();
    let mut candidates = Vec::new();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext_str.as_str()) {
                    let path_str = path.to_string_lossy().to_string();
                    if !existing_paths.contains(&path_str) {
                        candidates.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        return;
    }

    let total_candidates = candidates.len();
    let processed_count = Arc::new(AtomicUsize::new(0));
    let processed_counter_monitor = processed_count.clone();
    let app_handle_monitor = app.clone();
    
    thread::spawn(move || {
        let start_time = Instant::now();
        let mut last_check = Instant::now();
        let mut last_count = 0;
        let mut smoothed_rate: f64 = 0.0;

        loop {
            thread::sleep(Duration::from_millis(250));
            let current = processed_counter_monitor.load(Ordering::Relaxed);
            
            let now = Instant::now();
            let elapsed_total = now.duration_since(start_time).as_secs_f64();
            let elapsed_since_last = now.duration_since(last_check).as_secs_f64();
            
            let eta = if current > 0 && elapsed_total > 1.0 {
                let delta_items = (current - last_count) as f64;
                
                let instant_rate = if elapsed_since_last > 0.0 {
                    delta_items / elapsed_since_last
                } else {
                    0.0
                };

                if smoothed_rate == 0.0 {
                    smoothed_rate = instant_rate;
                } else {
                    smoothed_rate = 0.3 * instant_rate + 0.7 * smoothed_rate;
                }

                if smoothed_rate > 0.1 {
                    let remaining = total_candidates.saturating_sub(current);
                    Some((remaining as f64 / smoothed_rate) as u64)
                } else {
                    None
                }
            } else {
                None
            };

            last_count = current;
            last_check = now;

            let _ = app_handle_monitor.emit("indexing_progress", ProgressPayload {
                current,
                total: total_candidates,
                eta_seconds: eta
            });

            if current >= total_candidates {
                break;
            }
        }
    });

    let new_stickers: Vec<Option<(String, String, String, String, String)>> = candidates
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            let pack = path.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
            let ext_str = path.extension().unwrap().to_string_lossy().to_lowercase();
            
            let thumb_path = generate_thumbnail(path, &thumb_dir);
            
            processed_count.fetch_add(1, Ordering::Relaxed);

            Some((
                path_str,
                name,
                pack,
                ext_str,
                thumb_path.to_string_lossy().to_string()
            ))
        })
        .collect();

    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO stickers (path, name, pack, format, thumbnail_path) VALUES (?1, ?2, ?3, ?4, ?5)"
        ).unwrap();

        for item in new_stickers {
            if let Some((path, name, pack, ext, thumb)) = item {
                stmt.execute((&path, &name, &pack, &ext, &thumb)).unwrap_or_default();
            }
        }
    }
    tx.commit().unwrap();
    
    let _ = app.emit("indexing_progress", ProgressPayload {
        current: total_candidates,
        total: total_candidates,
        eta_seconds: Some(0)
    });
    
    thread::sleep(Duration::from_millis(500));
    let _ = app.emit("library_updated", ());
}

fn generate_thumbnail(src_path: &Path, thumb_dir: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(src_path.to_string_lossy().as_bytes());
    let hash = hex::encode(hasher.finalize());
    let dest_path = thumb_dir.join(format!("{}.webp", hash));

    if dest_path.exists() {
        return dest_path;
    }

    if let Ok(file) = std::fs::File::open(src_path) {
        let mut reader = std::io::BufReader::new(file);
        if let Ok(img) = image::load(&mut reader, image::ImageFormat::from_path(src_path).unwrap_or(image::ImageFormat::Png)) {
            let width = img.width();
            let height = img.height();
            
            let (target_width, target_height) = if width == height {
                (160, 160)
            } else if width > height {
                let w = (width as u32 * 160) / height as u32;
                (w, 160)
            } else {
                let h = (height as u32 * 160) / width as u32;
                (160, h)
            };

            let src_image = Image::from_vec_u8(
                width,
                height,
                img.to_rgba8().into_raw(),
                PixelType::U8x4,
            ).unwrap();

            let mut dst_image = Image::new(
                target_width,
                target_height,
                src_image.pixel_type(),
            );

            let mut resizer = Resizer::new();
            let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
            
            resizer.resize(&src_image, &mut dst_image, &options).unwrap();

            let mut result_buf = Cursor::new(Vec::new());
            image::write_buffer_with_format(
                &mut result_buf,
                dst_image.buffer(),
                target_width,
                target_height,
                image::ColorType::Rgba8,
                image::ImageFormat::WebP,
            ).unwrap();
            
            let _ = fs::write(&dest_path, result_buf.into_inner());
            return dest_path;
        }
    }
    src_path.to_path_buf()
}

// places the sticker in your textbox
#[tauri::command]
async fn select_sticker(app: AppHandle, path: String) -> Result<(), String> {
    let conn = get_db_conn(&app);
    let now = Utc::now().timestamp();
    let _ = conn.execute(
        "UPDATE stickers SET use_count = use_count + 1, last_used = ?1 WHERE path = ?2",
        [&now as &dyn ToSql, &path as &dyn ToSql],
    );

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    let path_buf = PathBuf::from(&path);
    let extension = path_buf.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if extension == "gif" {
        let _ = (|| -> Result<(), String> {
            let _clip = WinClipboard::new_attempts(10).map_err(|e| e.to_string())?;
            let files = vec![path.clone()];
            formats::FileList.write_clipboard(&files).map_err(|e| e.to_string())?;
            Ok(())
        })().map_err(|e| format!("Clipboard error: {}", e))?;
    } else {
        let app_dir = get_app_dir(&app);
        let thumb_dir = app_dir.join("thumbnails");
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let thumb_path = thumb_dir.join(format!("{}.webp", hash));

        let load_path = if thumb_path.exists() {
            thumb_path
        } else {
            PathBuf::from(&path)
        };

        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        let img = ImageReader::open(&load_path).map_err(|e| e.to_string())?.decode().map_err(|e| e.to_string())?;
        let rgba = img.into_rgba8(); 
        let image_data = ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: Cow::from(rgba.into_raw()),
        };
        clipboard.set_image(image_data).map_err(|e| e.to_string())?;
    }

    thread::sleep(Duration::from_millis(150));
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(Key::Control, Direction::Release);

    Ok(())
}

#[tauri::command]
async fn search_stickers(app: AppHandle, query: String, tab: String, limit: usize) -> Result<Vec<Sticker>, String> {
    let conn = get_db_conn(&app);
    let mut stmt;
    let mut stickers = Vec::new();

    let query_pattern = format!("%{}%", query);
    let limit_val = limit as i64; 

    let sql = if tab == "Recents" {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE use_count > 0 ORDER BY last_used DESC LIMIT ?1"
    } else if tab == "Favorites" {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE is_favorite = 1 AND name LIKE ?2 LIMIT ?1"
    } else if tab == "All" {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE name LIKE ?2 LIMIT ?1"
    } else {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE pack = ?3 AND name LIKE ?2 LIMIT ?1"
    };

    stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    let rows = if tab == "Recents" {
        stmt.query_map(&[&limit_val as &dyn ToSql], parse_sticker)
    } else if tab == "All" || tab == "Favorites" {
        stmt.query_map(&[&limit_val as &dyn ToSql, &query_pattern as &dyn ToSql], parse_sticker)
    } else {
        stmt.query_map(&[&limit_val as &dyn ToSql, &query_pattern as &dyn ToSql, &tab as &dyn ToSql], parse_sticker)
    };

    if let Ok(itr) = rows {
        for sticker in itr {
            if let Ok(s) = sticker {
                stickers.push(s);
            }
        }
    }

    Ok(stickers)
}

#[tauri::command]
async fn get_packs(app: AppHandle) -> Result<Vec<String>, String> {
    let conn = get_db_conn(&app);
    let mut stmt = conn.prepare("SELECT DISTINCT pack FROM stickers ORDER BY pack").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
    
    let mut packs = Vec::new();
    for pack in rows {
        if let Ok(p) = pack { packs.push(p); }
    }
    Ok(packs)
}

#[tauri::command]
fn toggle_favorite(app: AppHandle, path: String) -> bool {
    let conn = get_db_conn(&app);
    let is_fav: bool = conn.query_row(
        "SELECT is_favorite FROM stickers WHERE path = ?1", 
        [&path], 
        |row| row.get(0)
    ).unwrap_or(false);

    let new_val = if is_fav { 0 } else { 1 };
    let _ = conn.execute("UPDATE stickers SET is_favorite = ?1 WHERE path = ?2", [&new_val as &dyn ToSql, &path as &dyn ToSql]);
    
    !is_fav
}

fn get_app_dir(app_handle: &AppHandle) -> PathBuf {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    if !app_dir.exists() { let _ = fs::create_dir_all(&app_dir); }
    app_dir
}

#[tauri::command]
fn get_settings(app: AppHandle) -> AppSettings {
    load_settings_internal(&app)
}

fn load_settings_internal(app_handle: &AppHandle) -> AppSettings {
    let app_dir = get_app_dir(app_handle);
    let settings_path = app_dir.join("settings.json");
    
    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppSettings::default()
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: AppSettings) {
    let app_dir = get_app_dir(&app);
    let settings_path = app_dir.join("settings.json");
    let _ = fs::write(settings_path, serde_json::to_string(&settings).unwrap());
    
    #[cfg(target_os = "windows")]
    if let Some(window) = app.get_webview_window("main") {
        apply_theme_internal(&window, &settings.theme);
    }
}

#[tauri::command]
fn apply_theme(app: AppHandle, theme: String) {
    #[cfg(target_os = "windows")]
    if let Some(window) = app.get_webview_window("main") {
        apply_theme_internal(&window, &theme);
    }
}

#[cfg(target_os = "windows")]
fn apply_theme_internal(window: &tauri::WebviewWindow, theme: &str) {
    if theme == "mica" {
        let _ = apply_mica(window, None);
    } else {
        // Default to Acrylic (0,0,0,10) is a faint tint
        let _ = apply_acrylic(window, Some((0, 0, 0, 10))); 
    }
}

#[tauri::command]
fn wipe_data(app: AppHandle, data_type: String) -> bool {
    let conn = get_db_conn(&app);
    if data_type == "history" {
        let _ = conn.execute("UPDATE stickers SET use_count = 0, last_used = 0", []);
    } else if data_type == "favorites" {
        let _ = conn.execute("UPDATE stickers SET is_favorite = 0", []);
    } else if data_type == "db" {
        let _ = conn.execute("DELETE FROM stickers", []);
        let _ = conn.execute("VACUUM", []);
        let app_dir = get_app_dir(&app);
        let thumb_dir = app_dir.join("thumbnails");
        if thumb_dir.exists() {
            let _ = fs::remove_dir_all(&thumb_dir);
            let _ = fs::create_dir_all(&thumb_dir);
        }
    }
    true
}

fn resolve_sticker_path(app_handle: &AppHandle) -> PathBuf {
    let settings = load_settings_internal(app_handle);
    
    // If setting exists and is valid, use it
    if !settings.sticker_path.is_empty() {
        let path = PathBuf::from(&settings.sticker_path);
        if path.exists() {
            return path;
        }
    }

    // default is your pictures folder
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    return Path::new(&user_profile).join("Pictures\\Stickers");
}

#[tauri::command]
fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn parse_sticker(row: &rusqlite::Row) -> rusqlite::Result<Sticker> {
    Ok(Sticker {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        thumbnail_path: row.get(3).unwrap_or(row.get(2)?), // Fallback to main path if no thumb
        format: row.get(4)?,
        pack: row.get(5)?,
        is_favorite: row.get::<_, i32>(6)? == 1,
        width: row.get(7).unwrap_or(0),
        height: row.get(8).unwrap_or(0),
    })
}

fn handle_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
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