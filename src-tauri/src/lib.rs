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
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetGUIThreadInfo, GUITHREADINFO};

    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Ole::{
        SafeArrayAccessData, SafeArrayDestroy, SafeArrayUnaccessData,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern2, IUIAutomationTextRange,
        UIA_EditControlTypeId, UIA_TextPattern2Id,
    };

    unsafe {
        //println!("\n--- [DEBUG] STARTING CARET SEARCH ---");
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
            //println!(
            //    "[DEBUG] Win32 Caret found at Physical X:{}, Y:{}",
            //    pt.x, pt.y
            //);
            return Some((pt.x, pt.y));
        }

        //println!("[DEBUG] Win32 failed. Initializing UI Automation (COM)...");
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
                                        let c_elements = (*safearray_ptr).rgsabound[0].cElements;

                                        if c_elements >= 4 {
                                            let mut raw_data: *mut std::ffi::c_void =
                                                std::ptr::null_mut();

                                            if SafeArrayAccessData(safearray_ptr, &mut raw_data)
                                                .is_ok()
                                            {
                                                let data = raw_data as *const f64;

                                                let left = *data.offset(0);
                                                let top = *data.offset(1);
                                                let height = *data.offset(3);

                                                let _ = SafeArrayUnaccessData(safearray_ptr);
                                                let _ = SafeArrayDestroy(safearray_ptr);

                                                //println!("[DEBUG] UIA Caret geometry -> Left: {:.2}, Top: {:.2}, Height: {:.2}", left, top, height);
                                                return Some((left as i32, (top + height) as i32));
                                            }
                                        } else {
                                            //println!(
                                            //    "[DEBUG] SAFEARRAY was empty (No visible caret)."
                                            //);
                                        }
                                        let _ = SafeArrayDestroy(safearray_ptr);
                                    }
                                }
                            }
                        }
                    }
                }

                //println!(
                //    "[DEBUG] TextPattern2 not supported or no caret. Checking Control Type..."
                //);
                if let Ok(control_type) = focused.CurrentControlType() {
                    if control_type.0 == UIA_EditControlTypeId.0 {
                        if let Ok(has_focus) = focused.CurrentHasKeyboardFocus() {
                            if has_focus.as_bool() {
                                //println!("[DEBUG] Element IS a focused Edit control! Falling back to Mouse Cursor.");
                                let mut pt = POINT::default();
                                if GetCursorPos(&mut pt).is_ok() {
                                    return Some((pt.x, pt.y));
                                }
                            }
                        }
                    } else {
                        //println!("[DEBUG] Element is NOT a dedicated Edit control. Ignoring.");
                    }
                }
            }
        }
        //println!("[DEBUG] No caret or text box found in either system.");
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
                        if let Ok(mut lock) = state.custom_size.lock() {
                            *lock = Some(*size);
                        }
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
            //println!("\n===================================");
            //println!("[DEBUG] Shortcut ALT+. Pressed!");

            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    //println!("[DEBUG] Window is visible. Hiding it.");
                    let _ = window.hide();
                } else {
                    //println!("[DEBUG] Window is hidden. Calculating position...");
                    let state = app.state::<crate::models::AppState>();
                    let caret_pos = get_caret_position();

                    if let Some((x, y)) = caret_pos {
                        //println!("[DEBUG] Entering TEXTBOX Mode.");
                        state.is_centered_mode.store(false, Ordering::SeqCst);

                        let custom_size = if let Ok(lock) = state.custom_size.lock() {
                            *lock
                        } else {
                            None
                        };

                        if let Some(size) = custom_size {
                            let _ = window.set_size(size);
                        } else {
                            let _ = window.set_size(PhysicalSize::new(450, 500));
                        }

                        let mut final_x = x;
                        let mut final_y = y + 20;

                        //println!(
                        //    "[DEBUG] Searching for Monitor containing point ({}, {})",
                        //    x, y
                        //);
                        let mut target_monitor = None;

                        if let Ok(monitors) = window.available_monitors() {
                            for m in monitors {
                                let pos = m.position();
                                let size = m.size();
                                if x >= pos.x
                                    && x < pos.x + (size.width as i32)
                                    && y >= pos.y
                                    && y < pos.y + (size.height as i32)
                                {
                                    //println!("[DEBUG] Caret matches Monitor at Pos {:?}", pos);
                                    target_monitor = Some(m);
                                    break;
                                }
                            }
                        }

                        if target_monitor.is_none() {
                            //println!("[DEBUG] Caret is outside all known monitors! Falling back to active window monitor.");
                            target_monitor = window.current_monitor().unwrap_or(None);
                        }

                        if let Some(monitor) = target_monitor {
                            let m_pos = monitor.position();
                            let m_size = monitor.size();
                            let w_size = window.outer_size().unwrap_or(PhysicalSize::new(450, 500));

                            // Clamp Right Edge
                            if final_x + (w_size.width as i32) > m_pos.x + (m_size.width as i32) {
                                //println!("[DEBUG] Clamping X to prevent right-screen bleed.");
                                final_x = m_pos.x + (m_size.width as i32) - (w_size.width as i32);
                            }
                            // Clamp Bottom Edge
                            if final_y + (w_size.height as i32) > m_pos.y + (m_size.height as i32) {
                                //println!("[DEBUG] Clamping Y: Popping window ABOVE the caret.");
                                final_y = y - (w_size.height as i32) - 10;
                            }
                        }

                        //println!(
                        //    "[DEBUG] Final Physical Coordinates -> X: {}, Y: {}",
                        //    final_x, final_y
                        //);
                        let _ = window.set_position(PhysicalPosition::new(final_x, final_y));
                    } else {
                        //println!("[DEBUG] Entering WORKSPACE Mode (Center).");
                        state.is_centered_mode.store(true, Ordering::SeqCst);

                        let _ = window.set_size(LogicalSize::new(800, 600));
                        let _ = window.center();
                    }

                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("app_shown", ());
                    //println!("[DEBUG] Window shown successfully.");
                }
            }
        }
    }
}
