pub mod clipboard;
pub mod commands;
pub mod database;
pub mod library;
pub mod models;
pub mod utils;

use crate::models::AppState;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};

pub mod watcher;

// gets pos of cursor in textbox
fn get_caret_position() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{GetGUIThreadInfo, GUITHREADINFO};

    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    // FIX 1: SafeArray functions live in the Ole module
    use windows::Win32::System::Ole::{
        SafeArrayAccessData, SafeArrayDestroy, SafeArrayUnaccessData,
    };
    // FIX 2: Imported IUIAutomationTextRange
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern2, IUIAutomationTextRange,
        UIA_TextPattern2Id,
    };

    unsafe {
        let mut gui_info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };

        if GetGUIThreadInfo(0, &mut gui_info).is_ok() && gui_info.hwndCaret.0 != 0 {
            let mut pt = POINT {
                x: gui_info.rcCaret.left,
                y: gui_info.rcCaret.bottom,
            };
            let _ = ClientToScreen(gui_info.hwndCaret, &mut pt);
            return Some((pt.x, pt.y));
        }

        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        if let Ok(automation) =
            CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        {
            if let Ok(focused) = automation.GetFocusedElement() {
                if let Ok(pattern) =
                    focused.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id)
                {
                    let mut is_active = windows::Win32::Foundation::BOOL::from(false);
                    let mut caret_range: Option<IUIAutomationTextRange> = None;

                    if let Ok(_) = pattern.GetCaretRange(&mut is_active, &mut caret_range) {
                        if is_active.as_bool() {
                            if let Some(caret_range) = caret_range {
                                if let Ok(safearray_ptr) = caret_range.GetBoundingRectangles() {
                                    if !safearray_ptr.is_null() {
                                        let mut raw_data: *mut std::ffi::c_void =
                                            std::ptr::null_mut();

                                        if SafeArrayAccessData(safearray_ptr, &mut raw_data).is_ok()
                                        {
                                            let data = raw_data as *const f64;

                                            let left = *data.offset(0);
                                            let top = *data.offset(1);
                                            let _width = *data.offset(2);
                                            let height = *data.offset(3);

                                            let _ = SafeArrayUnaccessData(safearray_ptr);
                                            let _ = SafeArrayDestroy(safearray_ptr);

                                            return Some((left as i32, (top + height) as i32 + 10));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(handle_shortcut)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            let handle = app.handle().clone();
            let watcher_handler = handle.clone();
            watcher::spawn_watcher(watcher_handler);

            let app_dir = utils::get_app_dir(&handle);
            let db_path = app_dir.join("library.db");
            let thumb_dir = app_dir.join("thumbnails");
            if !thumb_dir.exists() {
                let _ = fs::create_dir_all(&thumb_dir);
            }

            database::init_db(&db_path).expect("Failed to init DB");
            app.manage(AppState {
                db_path: db_path.clone(),
                is_indexing: Arc::new(AtomicBool::new(false)),
                custom_size: Arc::new(Mutex::new(None)),
                is_centered_mode: Arc::new(AtomicBool::new(true)),
            });

            let settings = utils::load_settings(&handle);
            #[cfg(target_os = "windows")]
            utils::apply_theme_to_window(&window, &settings.theme);

            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Period);
            handle
                .global_shortcut()
                .register(shortcut)
                .expect("Failed to register global shortcut");

            let index_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                library::index_library(&index_handle, db_path, thumb_dir);
            });

            let resize_handle = handle.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(size) = event {
                    let state = resize_handle.state::<AppState>();
                    if !state.is_centered_mode.load(Ordering::SeqCst) {
                        *state.custom_size.lock().unwrap() = Some(*size);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_stickers,
            commands::select_sticker,
            commands::toggle_favorite,
            commands::hide_window,
            commands::get_settings,
            commands::save_settings,
            commands::wipe_data,
            commands::apply_theme,
            commands::refresh_library,
            commands::get_packs,
            commands::delete_sticker,
            commands::rename_sticker,
            commands::move_sticker,
            commands::update_sticker,
            library::cache_dropped_item,
            library::commit_sticker
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_shortcut(app: &tauri::AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        if shortcut.matches(Modifiers::ALT, Code::Period) {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let state = app.state::<crate::models::AppState>();

                    let caret_pos = get_caret_position();

                    if let Some((x, y)) = caret_pos {
                        // textbox mode
                        state.is_centered_mode.store(false, Ordering::SeqCst);

                        if let Some(size) = *state.custom_size.lock().unwrap() {
                            let _ = window.set_size(size);
                        } else {
                            let _ = window.set_size(PhysicalSize::new(450, 500));
                        }

                        let _ = window.set_position(PhysicalPosition::new(x, y));
                    } else {
                        // default mode
                        state.is_centered_mode.store(true, Ordering::SeqCst);

                        let _ = window.set_size(LogicalSize::new(800, 600));
                        let _ = window.center();
                    }

                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("app_shown", ());
                }
            }
        }
    }
}
