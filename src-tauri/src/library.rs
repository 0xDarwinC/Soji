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
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

//user specified in the future? 25mb for now...
const MAX_STICKER_SIZE: u64 = 25*1024*1024;

fn get_ffmpeg_path() -> PathBuf {
    let current_exe = std::env::current_exe().unwrap();
    let current_dir = current_exe.parent().unwrap();
    let mut ffmpeg_path = current_dir.join("ffmpeg-x86_64-pc-windows-msvc.exe");
    if !ffmpeg_path.exists() {
        ffmpeg_path = current_dir.join("../../bin/ffmpeg-x86_64-pc-windows-msvc.exe");
    }
    if !ffmpeg_path.exists() {
        ffmpeg_path = PathBuf::from("ffmpeg");
    }
    ffmpeg_path
}

fn get_video_duration(path_or_url: &str) -> Option<f64> {
    let ffmpeg_path = get_ffmpeg_path();
    #[cfg(target_os = "windows")]
    let mut cmd = Command::new(&ffmpeg_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new(&ffmpeg_path);

    let output = cmd.args(["-i", path_or_url]).output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Parse "Duration: 00:00:10.50"
    if let Some(dur_idx) = stderr.find("Duration: ") {
        let start = dur_idx + "Duration: ".len();
        if start + 11 <= stderr.len() {
            let dur_str = &stderr[start..start+11];
            let parts: Vec<&str> = dur_str.split(':').collect();
            if parts.len() == 3 {
                let h: f64 = parts[0].parse().unwrap_or(0.0);
                let m: f64 = parts[1].parse().unwrap_or(0.0);
                let s: f64 = parts[2].parse().unwrap_or(0.0);
                return Some(h * 3600.0 + m * 60.0 + s);
            }
        }
    }
    None
}

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
                if ["png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "mp4", "webm", "mov"].contains(&ext_str.as_str()) {
                    let is_video = ["mp4", "webm", "mov"].contains(&ext_str.as_str());
                    if is_video {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if metadata.len() > MAX_STICKER_SIZE {
                                continue;
                            }
                        }
                        if let Some(duration) = get_video_duration(&path.to_string_lossy()) {
                            if duration > 15.0 {
                                continue;
                            }
                        } else {
                            // If we can't get the duration, reject to be safe
                            continue;
                        }
                    }

                    let path_str = path.to_string_lossy().to_string();
                    found_paths.insert(path_str.clone());
                    if !existing_paths.contains(&path_str) {
                        candidates.push((path.to_path_buf(), true));
                    } else {
                        let mut hasher = Sha256::new();
                        hasher.update(path_str.as_bytes());
                        let hash = hex::encode(hasher.finalize());
                        
                        let is_animated = if ext_str == "gif" || ["mp4", "webm", "mov"].contains(&ext_str.as_str()) {
                            true
                        } else if ext_str == "webp" {
                            crate::utils::is_animated_webp(&path)
                        } else {
                            false
                        };
                        
                        let dest_ext = if is_animated { "gif" } else { "webp" };
                        let dest_path = thumb_dir.join(format!("{}.{}", hash, dest_ext));
                        
                        if !dest_path.exists() {
                            let wrong_ext = if is_animated { "webp" } else { "gif" };
                            let wrong_thumb = thumb_dir.join(format!("{}.{}", hash, wrong_ext));
                            if wrong_thumb.exists() {
                                let _ = std::fs::remove_file(wrong_thumb);
                            }
                            candidates.push((path.to_path_buf(), false));
                        }
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
    let new_stickers: Vec<Option<(String, String, String, String, String, bool)>> = candidates
        .par_iter()
        .map(|(path, is_new)| {
            let path_str = path.to_string_lossy().to_string();
            let mut name = path.file_stem().unwrap().to_string_lossy().to_string();
            if name.to_lowercase().ends_with(".heic") { name = name[..name.len()-5].to_string(); }
            if name.to_lowercase().ends_with(".heif") { name = name[..name.len()-5].to_string(); }
            
            let pack = path.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
            let ext_str = path.extension().unwrap().to_string_lossy().to_lowercase();
            
            let thumb_path = generate_thumbnail(path, &thumb_dir);
            processed_count.fetch_add(1, Ordering::Relaxed);

            Some((path_str, name, pack, ext_str, thumb_path.to_string_lossy().to_string(), *is_new))
        })
        .collect();

    // batch insert
    {
        let mut insert_stmt = tx.prepare(
            "INSERT INTO stickers (path, name, pack, format, thumbnail_path) VALUES (?1, ?2, ?3, ?4, ?5)"
        ).unwrap();
        let mut update_stmt = tx.prepare(
            "UPDATE stickers SET thumbnail_path = ?1 WHERE path = ?2"
        ).unwrap();

        for item in new_stickers {
            if let Some((path, name, pack, ext, thumb, is_new)) = item {
                if is_new {
                    let _ = insert_stmt.execute((&path, &name, &pack, &ext, &thumb));
                } else {
                    let _ = update_stmt.execute((&thumb, &path));
                }
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
    let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    
    let is_video = ["mp4", "webm", "mov"].contains(&ext.as_str());
    let is_animated = if is_video || ext == "gif" {
        true
    } else if ext == "webp" {
        crate::utils::is_animated_webp(src_path)
    } else {
        false
    };

    let dest_ext = if is_animated { "gif" } else { "webp" };
    let dest_path = thumb_dir.join(format!("{}.{}", hash, dest_ext));

    if dest_path.exists() { return dest_path; }

    if is_animated {
        let dim_result = image::ImageReader::open(src_path)
            .and_then(|r| r.with_guessed_format()?.into_dimensions().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)));
        
        let mut target_width = 160;
        let mut target_height = 160;
        
        if let Ok((width, height)) = dim_result {
            if width <= 160 && height <= 160 {
                target_width = width;
                target_height = height;
            } else if width == height {
                target_width = 160;
                target_height = 160;
            } else if width > height {
                target_width = (width as u32 * 160) / height as u32;
                target_height = 160;
            } else {
                target_height = (height as u32 * 160) / width as u32;
                target_width = 160;
            };

            if target_width == width && target_height == height && ext == dest_ext {
                let _ = fs::copy(src_path, &dest_path);
                return dest_path;
            }
        }
        
        let scale_filter = if is_video {
            "fps=15,scale=160:160:force_original_aspect_ratio=increase,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".to_string()
        } else {
            format!("scale={}:{},split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse", target_width, target_height)
        };

        let ffmpeg_path = get_ffmpeg_path();
        #[cfg(target_os = "windows")]
        let mut cmd = Command::new(&ffmpeg_path);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        
        #[cfg(not(target_os = "windows"))]
        let mut cmd = Command::new(&ffmpeg_path);

        let _ = cmd.args([
                "-i", src_path.to_str().unwrap(),
                "-vf", &scale_filter,
                "-threads", "1",
                "-y", dest_path.to_str().unwrap()
            ])
            .output();
            
        return dest_path;
    }

    let open_result = image::ImageReader::open(src_path);

    if let Ok(reader) = open_result {
        let load_result = reader.with_guessed_format().unwrap_or_else(|_| image::ImageReader::open(src_path).unwrap()).decode();
        
        if let Ok(img) = load_result {
            let width = img.width();
            let height = img.height();
            
            let (target_width, target_height) = 
            if width <= 160 && height <= 160 {
                (width, height)
            }
            else if width == height{
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
        
        let ext = payload.split('?').next().unwrap_or("").split('.').last().unwrap_or("").to_lowercase();
        let is_video_ext = ["mp4", "webm", "mov"].contains(&ext.as_str());
        if is_video_ext {
            if let Some(duration) = get_video_duration(&payload) {
                if duration > 15.0 {
                    return Err("Video is too long! Limit is 15 seconds.".to_string());
                }
            }
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        let mut response = client.get(&payload)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Failed to download image. Status: {}", response.status()));
        }

        if let Some(len) = response.content_length() {
            if len > MAX_STICKER_SIZE {
                return Err(format!("File too large! =^[ Limit is {}MB...", MAX_STICKER_SIZE / 1024 / 1024));
            }
        }

        use std::io::{Read, Write};
        let mut file = std::fs::File::create(&initial_temp_path).map_err(|e| format!("Failed to create temp file: {}", e))?;
        let mut buffer = [0; 8192];
        let mut total_bytes: u64 = 0;

        loop {
            let bytes_read = response.read(&mut buffer).map_err(|e| format!("Network read error: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            total_bytes += bytes_read as u64;
            if total_bytes > MAX_STICKER_SIZE {
                drop(file);
                let _ = std::fs::remove_file(&initial_temp_path);
                return Err(format!("File exceeded size limit of {}MB", MAX_STICKER_SIZE / 1024 / 1024));
            }
            file.write_all(&buffer[..bytes_read]).map_err(|e| format!("Failed to write to temp file: {}", e))?;
        }
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

        // final size check for safety
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > MAX_STICKER_SIZE {
            return Err(format!("Local file too large. Limit is {}MB", MAX_STICKER_SIZE / 1024 / 1024));
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let is_video_ext = ["mp4", "webm", "mov"].contains(&ext.as_str());
        if is_video_ext {
            if let Some(duration) = get_video_duration(&decoded_path) {
                if duration > 15.0 {
                    return Err("Video is too long! Limit is 15 seconds.".to_string());
                }
            }
        }

        std::fs::copy(path, &initial_temp_path).map_err(|e| format!("Failed to copy local file: {}", e))?;
    }

    let kind = infer::get_from_path(&initial_temp_path)
        .map_err(|e| format!("Failed to inspect file: {}", e))?
        .ok_or("Unknown file type. Could not detect image format.")?;

    let mime = kind.mime_type();
    let detected_ext = kind.extension();

    println!("Detected mime: {}, ext: {}", mime, detected_ext);
    
    let is_video = mime.starts_with("video/");
    let is_image = mime.starts_with("image/") && mime != "image/svg+xml";

    if !is_image && !is_video {
         let _ = std::fs::remove_file(&initial_temp_path);
         return Err(format!("Invalid file type: {}. Only raster images and videos are supported.", mime));
    }

    if is_video {
        if let Some(duration) = get_video_duration(&initial_temp_path.to_string_lossy()) {
            if duration > 15.0 {
                let _ = std::fs::remove_file(&initial_temp_path);
                return Err("Video is too long! Limit is 15 seconds.".to_string());
            }
        } else {
            let _ = std::fs::remove_file(&initial_temp_path);
            return Err("Failed to determine video duration.".to_string());
        }
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
println!("Commit Sticker Requested: Name='{}', Pack='{}'", name, pack);

    // paths
    let root_dir = resolve_sticker_path(&app);
    let app_dir = get_app_dir(&app);
    let db_path = app_dir.join("library.db");
    
    // resolve dir
    let pack_dir = if let Ok(conn) = Connection::open(&db_path) {
        match database::get_pack_path(&conn, &pack) {
            Ok(Some(existing_file_path)) => {
                let p = PathBuf::from(existing_file_path);
                p.parent()
                 .map(|parent| parent.to_path_buf())
                 .unwrap_or_else(|| root_dir.join(&pack))
            },
            _ => root_dir.join(&pack) // create if dne
        }
    } else {
        root_dir.join(&pack)
    };

    if !pack_dir.exists() {
        std::fs::create_dir_all(&pack_dir).map_err(|e| format!("Create dir failed: {}", e))?;
    }

    let canonical_root = fs::canonicalize(&root_dir).map_err(|e| format!("Root invalid: {}", e))?;
    let canonical_pack = fs::canonicalize(&pack_dir).map_err(|e| format!("Pack invalid: {}", e))?;

    if !canonical_pack.starts_with(&canonical_root) {
        return Err("Security Violation: Cannot save sticker outside of library root.".to_string());
    }

    let source_path = Path::new(&temp_path);
    if !source_path.exists() { return Err("Source temp file missing".to_string()); }

    let ext = source_path.extension()
        .and_then(|e| e.to_str())
        .ok_or("Temp file missing extension")?;

    let safe_name: String = name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect();
    let safe_name = safe_name.trim();
    if safe_name.is_empty() { return Err("Invalid sticker name".to_string()); }

    let target_filename = format!("{}.{}", safe_name, ext);
    let target_path = pack_dir.join(&target_filename);

    if target_path.exists() {
        return Err(format!("File '{}' already exists in pack '{}'", target_filename, pack));
    }

    if let Err(_) = std::fs::rename(source_path, &target_path) {
        std::fs::copy(source_path, &target_path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(source_path);
    }

    let thumb_dir = app_dir.join("thumbnails");
    let app_handle = app.clone();
    std::thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        index_library(&app_handle, db_path, thumb_dir);
    });

    Ok(target_path.to_string_lossy().to_string())
}