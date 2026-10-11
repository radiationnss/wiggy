use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Default)]
pub struct CaptureState {
    pub busy: AtomicBool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Screen,
    Region,
    Window,
}

#[derive(Serialize)]
pub struct Screenshot {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    pub label: &'static str,
}

struct RestorePopup(AppHandle);

impl Drop for RestorePopup {
    fn drop(&mut self) {
        let app = self.0.clone();
        self.0
            .state::<CaptureState>()
            .busy
            .store(false, Ordering::SeqCst);
        let _ = self.0.run_on_main_thread(move || {
            if let Err(error) = crate::window::open(&app, false, false) {
                app.state::<crate::AppState>().record_error(error);
            }
        });
    }
}

#[tauri::command]
pub async fn capture_screen(
    window: WebviewWindow,
    app: AppHandle,
    target: Target,
) -> Result<Option<Screenshot>, String> {
    if window.label() != "palette" {
        return Err("Screen capture is only available in the local palette".into());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, target);
        Err("Screen capture currently requires Hyprland, grim and slurp".into())
    }
    #[cfg(target_os = "linux")]
    {
        let state = app.state::<CaptureState>();
        state
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "A screen capture is already in progress")?;
        let _restore = RestorePopup(app.clone());
        crate::window::hide(&app)?;
        crate::platform::linux::capture::capture(target).await
    }
}
