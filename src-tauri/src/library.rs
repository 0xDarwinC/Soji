use crate::models::{IndexingGuard, ProgressPayload, AppState};
use crate::utils::{get_app_dir, resolve_sticker_path};
use tauri::{AppHandle, Manager, Emitter};
use walkdir::WalkDir;
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use rusqlite::Connection;
use fast_image_resize as fr;
use fr::{Resizer, ResizeOptions, ResizeAlg, FilterType, PixelType};
use fr::images::Image;
use sha2::{Sha256, Digest};
use rayon::prelude::*;
use tokio::time::Instant;
use uuid::Uuid;
use reqwest::blocking::Client;
use url::Url;
use infer;
use crate::database;

pub fn index_library(app: &AppHandle, db_path: PathBuf, thumb_dir: PathBuf) {
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

    // scan dir
    let walker = WalkDir::new(sticker_root).into_iter();
    let mut found_paths = HashSet::new();
    let mut candidates = Vec::new();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ["png", "jpg", "jpeg", "gif", "webp", "heic", "heif"].contains(&ext_str.as_str()) {
                    let path_str = path.to_string_lossy().to_string();
                    found_paths.insert(path_str.clone());
                    if !existing_paths.contains(&path_str) {
                        candidates.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    let to_delete: Vec<String> = existing_paths
        .difference(&found_paths)
        .cloned()
        .collect();
    let tx = conn.transaction().unwrap();
    if !to_delete.is_empty() {
        for path in to_delete {
            let _ = tx.execute("DELETE FROM stickers WHERE path = ?1", [&path]);
        }
    }

    // progress diagnostics
    let total_candidates = candidates.len();
    let processed_count = Arc::new(AtomicUsize::new(0));
    let processed_counter_monitor = processed_count.clone();
    let app_handle_monitor = app.clone();
    
    thread::spawn(move || {
        let mut last_check = Instant::now();
        let mut last_count = 0;
        let mut smoothed_rate: f64 = 0.0;

        loop {
            thread::sleep(Duration::from_millis(250));
            let current = processed_counter_monitor.load(Ordering::Relaxed);
            let now = Instant::now();
            let elapsed_since_last = now.duration_since(last_check).as_secs_f64();   
            let eta = if current > 0 {
                let delta_items = (current - last_count) as f64;
                let instant_rate = if elapsed_since_last > 0.0 { delta_items / elapsed_since_last } else { 0.0 };
                    
                if smoothed_rate == 0.0 { smoothed_rate = instant_rate; } 
                else { smoothed_rate = 0.3 * instant_rate + 0.7 * smoothed_rate; }

                if smoothed_rate > 0.1 {
                    let remaining = total_candidates.saturating_sub(current);
                    Some((remaining as f64 / smoothed_rate) as u64)
                } else { None }
            } else { None };

            last_count = current;
            last_check = now;

            let _ = app_handle_monitor.emit("indexing_progress", ProgressPayload {
                current,
                total: total_candidates,
                eta_seconds: eta
            });

            if current >= total_candidates { break; }
        }
    });

    // process in parallel
    let new_stickers: Vec<Option<(String, String, String, String, String)>> = candidates
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();
            let mut name = path.file_stem().unwrap().to_string_lossy().to_string();
            if name.to_lowercase().ends_with(".heic") { name = name[..name.len()-5].to_string(); }
            if name.to_lowercase().ends_with(".heif") { name = name[..name.len()-5].to_string(); }
            
            let pack = path.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
            let ext_str = path.extension().unwrap().to_string_lossy().to_lowercase();
            
            let thumb_path = generate_thumbnail(path, &thumb_dir);
            processed_count.fetch_add(1, Ordering::Relaxed);

            Some((path_str, name, pack, ext_str, thumb_path.to_string_lossy().to_string()))
        })
        .collect();

    // batch insert
    {
        let mut stmt = tx.prepare(
            "INSERT INTO stickers (path, name, pack, format, thumbnail_path) VALUES (?1, ?2, ?3, ?4, ?5)"
        ).unwrap();

        for item in new_stickers {
            if let Some((path, name, pack, ext, thumb)) = item {
                let _ = stmt.execute((&path, &name, &pack, &ext, &thumb));
            }
        }
    }
    tx.commit().unwrap();
    
    let _ = app.emit("indexing_progress", ProgressPayload {
        current: 0,
        total: 0,
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

    if dest_path.exists() { return dest_path; }

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

            let src_image = Image::from_vec_u8(width, height, img.to_rgba8().into_raw(), PixelType::U8x4).unwrap();
            let mut dst_image = Image::new(target_width, target_height, src_image.pixel_type());
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

// caches drag and drop object
#[tauri::command]
pub fn cache_dropped_item(app: tauri::AppHandle, payload: String) -> Result<serde_json::Value, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let temp_dir = app_data_dir.join("temp_staging");
    
    if !temp_dir.exists() {
        std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    }

    let temp_id = Uuid::new_v4().to_string();
    let initial_temp_path = temp_dir.join(&temp_id);
    let is_url = Url::parse(&payload).is_ok() && (payload.starts_with("http://") || payload.starts_with("https://"));

    if is_url {
        println!("Attempting to download URL: {}", payload);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client.get(&payload)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        if !response.status().is_success() {
             return Err(format!("Failed to download image. Status: {}", response.status()));
        }

        let bytes = response.bytes().map_err(|e| format!("Failed to read bytes: {}", e))?;
        std::fs::write(&initial_temp_path, &bytes).map_err(|e| format!("Failed to write temp file: {}", e))?;
    } else {
        let clean_path = if payload.starts_with("file:///") {
            payload.replace("file:///", "")
        } else if payload.starts_with("file://") {
             payload.replace("file://", "")
        } else {
            payload.clone()
        };
        
        let decoded_path = urlencoding::decode(&clean_path).map_err(|e| e.to_string())?.into_owned();
        let path = Path::new(&decoded_path);

        if !path.exists() || !path.is_file() {
             return Err(format!("File does not exist locally or is not a file: {}", decoded_path));
        }
        std::fs::copy(path, &initial_temp_path).map_err(|e| format!("Failed to copy local file: {}", e))?;
    }

    let kind = infer::get_from_path(&initial_temp_path)
        .map_err(|e| format!("Failed to inspect file: {}", e))?
        .ok_or("Unknown file type. Could not detect image format.")?;

    let mime = kind.mime_type();
    let detected_ext = kind.extension();

    println!("Detected mime: {}, ext: {}", mime, detected_ext);
    if !mime.starts_with("image/") || mime == "image/svg+xml" {
         let _ = std::fs::remove_file(&initial_temp_path);
         return Err(format!("Invalid file type: {}. Only raster images (PNG, JPG, GIF, WEBP) are supported.", mime));
    }

    let final_temp_path = temp_dir.join(format!("{}.{}", temp_id, detected_ext));
    std::fs::rename(&initial_temp_path, &final_temp_path).map_err(|e| format!("Failed to rename temp file: {}", e))?;

    Ok(serde_json::json!({
        "temp_path": final_temp_path.to_string_lossy().into_owned(),
        "detected_extension": detected_ext,
        "source_type": if is_url { "url" } else { "local" }
    }))
}

/// commits the temp staged file to lib
#[tauri::command]
pub fn commit_sticker(app: tauri::AppHandle, temp_path: String, name: String, pack: String) -> Result<String, String> {
    println!("Commit Sticker Requested: Name='{}', Pack='{}', Source='{}'", name, pack, temp_path);

    let root_dir = resolve_sticker_path(&app);
    let source_path = Path::new(&temp_path);

    if !source_path.exists() {
         return Err(format!("Staging file missing at: {}", temp_path));
    }

    let app_dir = get_app_dir(&app);
    let db_path = app_dir.join("library.db");
    
    let pack_dir = if let Ok(conn) = Connection::open(&db_path) {
        match database::get_pack_path(&conn, &pack) {
            Ok(Some(existing_path_str)) => {
                PathBuf::from(existing_path_str).parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| root_dir.join(&pack))
            },
            _ => root_dir.join(&pack)
        }
    } else {
        root_dir.join(&pack)
    };

    let clean_root = fs::canonicalize(&root_dir).unwrap_or(root_dir.clone());
    if !pack_dir.exists() {
        std::fs::create_dir_all(&pack_dir).map_err(|e| format!("Failed to create pack directory: {}", e))?;
    }
    let clean_target_pack = fs::canonicalize(&pack_dir).map_err(|e| format!("Path resolution error: {}", e))?;

    if !clean_target_pack.starts_with(&clean_root) {
        return Err("Security Violation: Cannot save sticker outside of library directory.".to_string());
    }

    let ext = source_path.extension()
        .and_then(|e| e.to_str())
        .ok_or("Temp file has missing extension error")?;

    let safe_name: String = name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect();
    let safe_name = safe_name.trim();

    if safe_name.is_empty() {
         return Err("Sticker name cannot be empty or special characters only.".to_string());
    }
    
    let target_filename = format!("{}.{}", safe_name, ext);
    let target_path = pack_dir.join(&target_filename);

    if target_path.exists() {
        return Err(format!("A sticker named '{}' already exists in pack '{}'.", safe_name, pack));
    }

    if let Err(_e) = std::fs::rename(source_path, &target_path) {
        std::fs::copy(source_path, &target_path).map_err(|err| format!("Failed to copy to library: {}", err))?;
        let _ = std::fs::remove_file(source_path);
    }

    let thumb_dir = app_dir.join("thumbnails");
    let app_handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        index_library(&app_handle, db_path, thumb_dir);
    });

    Ok(target_path.to_string_lossy().into_owned())
}