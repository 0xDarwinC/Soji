use tauri::{AppHandle, Manager, Emitter, State};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use window_vibrancy::{apply_acrylic, apply_mica};
use std::path::Path;
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};
use arboard::{Clipboard, ImageData};
use image::ImageReader;
use std::borrow::Cow;
use std::thread;
use std::time::Duration;
use enigo::{Enigo, Key, Keyboard, Settings, Direction};
use clipboard_win::{formats, Clipboard as WinClipboard, Setter};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::clangd::ClangdMatcher;
use std::fs;
use std::collections::HashSet;
use chrono::{Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri_plugin_dialog;
use std::sync::Mutex;

// data models
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sticker {
    name: String,
    path: String,
    format: String,
    pack: String,
    #[serde(skip)]
    score: i64, // used for relevance search
    is_favorite: bool,
    rec_score: f64,
}

// used in recents classification
#[derive(Debug, Serialize, Deserialize, Clone)]
struct HistoryEntry {
    count: u64,
    last_used: i64,
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

// fast access index
struct AppState {
    stickers: Mutex<Vec<Sticker>>,
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
        .manage(AppState {
            stickers: Mutex::new(Vec::new()),
        })
        .invoke_handler(tauri::generate_handler![list_stickers, 
            select_sticker, 
            search_stickers, 
            toggle_favorite, 
            hide_window,
            get_settings,
            save_settings,
            wipe_data,
            apply_theme,
            refresh_library,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            let settings = load_settings_internal(app.handle());
            #[cfg(target_os = "windows")]
            apply_theme_internal(&window, &settings.theme);

            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Period);
            app.global_shortcut().register(shortcut).expect("Failed to register global shortcut");
            
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                index_stickers(&app_handle);
            });
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
                    let _ = window.emit("app_shown", ());
                }
            }
        }
    }
}

fn get_app_dir(app_handle: &AppHandle) -> PathBuf {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    if !app_dir.exists() { let _ = fs::create_dir_all(&app_dir); }
    app_dir
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

// decay func: count / (hrssince+2)^1.5
fn calc_recency(entry: &HistoryEntry, now: i64) -> f64 {
    let hours_since = (now - entry.last_used).max(0) as f64 / 3600.0;
    (entry.count as f64) / (hours_since + 2.0).powf(1.5)
}

// propagates stickers into index
fn index_stickers(app_handle: &AppHandle) {
    let sticker_path = resolve_sticker_path(app_handle);
    let settings = load_settings_internal(app_handle);
    let app_dir = get_app_dir(app_handle);
    
    let mut new_stickers = Vec::new();

    // load recs
    let hist_path = app_dir.join("history.json");
    let history: HashMap<String, HistoryEntry> = if hist_path.exists() {
        let content = fs::read_to_string(&hist_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let now = Utc::now().timestamp();
    let score_map: HashMap<String, f64> = history.iter()
        .map(|(path, entry)| (path.clone(), calc_recency(entry, now)))
        .collect();
    let mut scored_list: Vec<(&String, f64)> = score_map.iter().map(|(k,v)| (k,*v)).collect();
    scored_list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    let recent_paths: HashSet<String> = scored_list.into_iter()
        .take(settings.recents_limit)
        .map(|(p, _)| p.clone())
        .collect();

    // load favs
    let fav_path = app_dir.join("favorites.json");
    let favorites: HashSet<String> = if fav_path.exists() {
        let content = fs::read_to_string(&fav_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashSet::new()
    };

    if sticker_path.exists() {
        for entry in WalkDir::new(sticker_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext_str.as_str()) {
                        let path_str = path.to_string_lossy().to_string();

                        let parent_name = path.parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();

                        let recency = *score_map.get(&path_str).unwrap_or(&0.0);
                        // If it's in the top N recents, keep score. Else 0.
                        let final_recency = if recent_paths.contains(&path_str) { recency } else { 0.0 };

                        new_stickers.push(Sticker {
                            name: path.file_stem().unwrap().to_string_lossy().to_string(),
                            path: path_str.clone(),
                            format: ext_str,
                            pack: parent_name,
                            score: 0,
                            is_favorite: favorites.contains(&path_str),
                            rec_score: final_recency,
                        });
                    }
                }
            }
        }
    }

    // update global state
    let state = app_handle.state::<AppState>();
    let mut stickers_guard = state.stickers.lock().unwrap();
    *stickers_guard = new_stickers;
    
    let _ = app_handle.emit("library_updated", ());
}

#[tauri::command]
async fn list_stickers(state: State<'_, AppState>) -> Result<Vec<Sticker>, String> {
    let stickers = state.stickers.lock().unwrap();
    Ok(stickers.clone())
}

// places the sticker in your textbox
#[tauri::command]
async fn select_sticker(app: AppHandle, path: String) -> Result<(), String> {
    // update recency list due to sticker use
    {
        let store_path = app.path().app_data_dir().unwrap();
        if !store_path.exists() { let _ = fs::create_dir_all(&store_path); }
        let hist_path = store_path.join("history.json");
        
        let mut history: HashMap<String, HistoryEntry> = if hist_path.exists() {
            let content = fs::read_to_string(&hist_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let now = Utc::now().timestamp();
        let entry = history.entry(path.clone()).or_insert(HistoryEntry { count: 0, last_used: 0 });
        entry.count += 1;
        entry.last_used = now;

        if let Ok(json) = serde_json::to_string(&history) {
             let _ = fs::write(hist_path, json);
        }
    }
    
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

// search for stickers using fuzzy search
#[tauri::command]
async fn search_stickers(state: State<'_, AppState>, query: String) -> Result<Vec<Sticker>, String> {
    let stickers_guard = state.stickers.lock().unwrap();
    let all_stickers = stickers_guard.clone();
    
    if query.is_empty() {
        return Ok(all_stickers);
    }

    let matcher = ClangdMatcher::default();
    let mut matches: Vec<Sticker> = all_stickers
        .into_iter()
        .filter_map(|mut sticker| {
            matcher.fuzzy_match(&sticker.name, &query).map(|score| {
                sticker.score = score;
                sticker
            })
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    Ok(matches)
}

#[tauri::command]
async fn refresh_library(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        index_stickers(&app);
    });
}

#[tauri::command]
fn toggle_favorite(app: AppHandle, path: String) -> bool {
    let store_path = app.path().app_data_dir().unwrap();
    // Ensure folder exists
    if !store_path.exists() {
        let _ = fs::create_dir_all(&store_path);
    }
    let file_path = store_path.join("favorites.json");

    // Read existing
    let mut favorites: HashSet<String> = if file_path.exists() {
        let content = fs::read_to_string(&file_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashSet::new()
    };

    // Toggle
    let is_fav = if favorites.contains(&path) {
        favorites.remove(&path);
        false
    } else {
        favorites.insert(path);
        true
    };

    // Save
    let _ = fs::write(file_path, serde_json::to_string(&favorites).unwrap());
    
    is_fav
}

#[tauri::command]
fn get_settings(app: AppHandle) -> AppSettings {
    load_settings_internal(&app)
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
    let app_dir = get_app_dir(&app);
    let file_name = if data_type == "history" { "history.json" } else { "favorites.json" };
    let file_path = app_dir.join(file_name);
    
    if file_path.exists() {
        return fs::remove_file(file_path).is_ok();
    }
    true
}

#[tauri::command]
fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}