use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use crate::{utils, library};

// watchdog for library updates outside of app

pub fn spawn_watcher(app: AppHandle) {
    thread::spawn(move || {
        let (tx, rx) = channel();
        
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to init watcher: {}", e);
                return;
            }
        };

        let watch_path = utils::resolve_sticker_path(&app);
        
        if let Err(e) = watcher.watch(&watch_path, RecursiveMode::Recursive) {
            eprintln!("Failed to watch path: {}", e);
            return;
        }

        loop {
            match rx.recv() {
                Ok(Ok(_event)) => {
                    let settle_time = Duration::from_secs(1);
                    
                    loop {
                        match rx.recv_timeout(settle_time) {
                            Ok(Ok(_)) => {
                                continue; 
                            },
                            Err(RecvTimeoutError::Timeout) => {
                                break;
                            },
                            Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }

                    // lib refresh
                    let app_dir = utils::get_app_dir(&app);
                    let db_path = app_dir.join("library.db");
                    let thumb_dir = app_dir.join("thumbnails");
                    
                    library::index_library(&app, db_path, thumb_dir);
                },
                Ok(Err(e)) => eprintln!("Watch error: {}", e),
                Err(_) => return,
            }
        }
    });
}