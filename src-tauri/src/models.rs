use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sticker {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub thumbnail_path: String,
    pub format: String,
    pub pack: String,
    pub is_favorite: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub sticker_path: String,
    pub recents_limit: usize,
    pub theme: String,
    pub disable_animations: bool,
    pub max_items: i32,
    pub run_on_startup: bool,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sticker_path: "".to_string(),
            recents_limit: 18,
            theme: "acrylic".to_string(),
            disable_animations: false,
            max_items: 200,
            run_on_startup: true,
        }
    }
}

pub struct AppState {
    pub db_path: PathBuf,
    pub is_indexing: Arc<AtomicBool>,
}

#[derive(Clone, Serialize)]
pub struct ProgressPayload {
    pub current: usize,
    pub total: usize,
    pub eta_seconds: Option<u64>,
}

pub struct IndexingGuard(pub Arc<AtomicBool>);
impl Drop for IndexingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}