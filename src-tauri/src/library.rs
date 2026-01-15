use crate::models::{IndexingGuard, ProgressPayload, AppState};
use crate::utils::resolve_sticker_path;
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
    let mut candidates = Vec::new();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ["png", "jpg", "jpeg", "gif", "webp", "heic", "heif"].contains(&ext_str.as_str()) {
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

    // progress diagnostics
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