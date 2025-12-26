use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use window_vibrancy::{apply_acrylic};
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(handle_shortcut)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![list_stickers, select_sticker, search_stickers, toggle_favorite, hide_window])
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
                    let _ = window.emit("app_shown", ());
                }
            }
        }
    }
}

// decay func: count / (hrssince+2)^1.5
fn calc_recency(entry: &HistoryEntry, now: i64) -> f64 {
    let hours_since = (now - entry.last_used).max(0) as f64 / 3600.0;
    (entry.count as f64) / (hours_since + 2.0).powf(1.5)
}

// propagates stickers
fn get_all_stickers(app_handle: &AppHandle) -> Vec<Sticker> {
    // TODO: Change this to user specified dir
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let sticker_path = Path::new(&user_profile).join("Pictures\\Stickers");
    let mut stickers = Vec::new();

    let app_dir = app_handle.path().app_data_dir().unwrap();
    if !app_dir.exists() { let _ = fs::create_dir_all(&app_dir); }

    // load recents
    let hist_path = app_dir.join("history.json");
    let history: HashMap<String, HistoryEntry> = if hist_path.exists() {
        let content = fs::read_to_string(&hist_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // pick top 18 recents (will fill first 3 rows)
    let now = Utc::now().timestamp();
    let score_map: HashMap<String, f64> = history.iter()
        .map(|(path, entry)| (path.clone(), calc_recency(entry, now)))
        .collect();

    // load favs
    let store_path = app_dir.join("favorites.json");
    let favorites: HashSet<String> = if store_path.exists() {
        let content = fs::read_to_string(&store_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashSet::new()
    };

    if !sticker_path.exists() { return stickers; }

    for entry in WalkDir::new(sticker_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                let ext_str = extension.to_string_lossy().to_lowercase();
                if ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext_str.as_str()) {
                    let path_str = path.to_string_lossy().to_string();

                    // organize into packs
                    let parent_name = path.parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    let recency = *score_map.get(&path_str).unwrap_or(&0.0);

                    stickers.push(Sticker {
                        name: path.file_stem().unwrap().to_string_lossy().to_string(),
                        path: path_str.clone(),
                        format: ext_str,
                        pack: parent_name,
                        score: 0,
                        is_favorite: favorites.contains(&path_str),
                        rec_score: recency,
                    });
                }
            }
        }
    }
    stickers
}

// list stickers from specified dir
#[tauri::command]
fn list_stickers(app: AppHandle) -> Vec<Sticker> {
    get_all_stickers(&app)
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
fn search_stickers(app: AppHandle, query: String) -> Vec<Sticker> {
    let all_stickers = get_all_stickers(&app);
    
    if query.is_empty() {
        return all_stickers;
    }

    let matcher = ClangdMatcher::default();
    let mut matches: Vec<Sticker> = all_stickers
        .into_iter()
        .filter_map(|mut sticker| {
            // fuzzy match by filename
            matcher.fuzzy_match(&sticker.name, &query).map(|score| {
                sticker.score = score;
                sticker
            })
        })
        .collect();

    // sort by relevance
    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
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
fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}