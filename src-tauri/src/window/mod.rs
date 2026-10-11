pub mod geometry;

use crate::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

pub const WIDTH: u32 = 480;
pub const COMPACT_HEIGHT: u32 = 180;
pub const CHAT_HEIGHT: u32 = 400;
pub const SETTINGS_HEIGHT: u32 = 400;

pub fn resize(app: &AppHandle, height: u32) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.popup_height.store(height, Ordering::SeqCst);
    if !state.visible.load(Ordering::SeqCst) {
        return Ok(());
    }
    let window = app
        .get_webview_window("palette")
        .ok_or("Palette window is missing")?;
    window
        .set_size(tauri::LogicalSize::new(f64::from(WIDTH), f64::from(height)))
        .map_err(|error| error.to_string())?;
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    place(app, generation, height, None);
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.generation.fetch_add(1, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("palette") {
        window.hide().map_err(|error| error.to_string())?;
    }
    state.visible.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn open(app: &AppHandle, settings: bool, toggle: bool) -> Result<(), String> {
    if app
        .state::<crate::capture::CaptureState>()
        .busy
        .load(Ordering::SeqCst)
    {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let window = app
        .get_webview_window("palette")
        .ok_or("Palette window is missing")?;
    if toggle && state.visible.load(Ordering::SeqCst) && window.is_focused().unwrap_or(false) {
        return hide(app);
    }
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let previous_settings = state.settings_view.swap(settings, Ordering::SeqCst);
    let height = if settings {
        SETTINGS_HEIGHT
    } else if previous_settings {
        COMPACT_HEIGHT
    } else {
        state.popup_height.load(Ordering::SeqCst)
    };
    state.popup_height.store(height, Ordering::SeqCst);
    window
        .set_size(tauri::LogicalSize::new(f64::from(WIDTH), f64::from(height)))
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    state.visible.store(true, Ordering::SeqCst);
    place(app, generation, height, Some(settings));
    Ok(())
}

fn place(app: &AppHandle, generation: u64, height: u32, view: Option<bool>) {
    let handle = app.clone();
    std::thread::spawn(move || {
        #[cfg(target_os = "linux")]
        {
            let mut last_error = None;
            for _attempt in 0..12 {
                if handle.state::<AppState>().generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                match crate::platform::linux::Hyprland::connect().and_then(|adapter| {
                    let position = handle
                        .state::<AppState>()
                        .settings
                        .lock()
                        .map_err(|error| error.to_string())?
                        .popup_position;
                    adapter.place_popup(position, height, view.is_some())
                }) {
                    Ok(()) => {
                        last_error = None;
                        break;
                    }
                    Err(error) => {
                        last_error = Some(error);
                        std::thread::sleep(std::time::Duration::from_millis(40));
                    }
                }
            }
            if let Some(error) = last_error {
                handle.state::<AppState>().record_error(error);
            }
        }
        let callback_app = handle.clone();
        if let Err(error) = handle.run_on_main_thread(move || {
            let state = callback_app.state::<AppState>();
            if state.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            if let Some(window) = callback_app.get_webview_window("palette") {
                if let Some(settings) = view {
                    if let Err(error) = window.set_focus() {
                        state.record_error(error.to_string());
                    }
                    if let Err(error) = window.emit(
                        if settings {
                            "settings-opened"
                        } else {
                            "popup-opened"
                        },
                        (),
                    ) {
                        state.record_error(error.to_string());
                    }
                }
            }
        }) {
            handle.state::<AppState>().record_error(error.to_string());
        }
    });
}
