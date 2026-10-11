mod agent;
mod capture;
mod config;
mod platform;
pub mod trigger;
mod window;

use config::Settings;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Mutex,
    },
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

struct AppState {
    settings: Mutex<Settings>,
    config_path: PathBuf,
    error: Mutex<Option<String>>,
    visible: AtomicBool,
    settings_view: AtomicBool,
    generation: AtomicU64,
    popup_height: AtomicU32,
    running: AtomicBool,
    trigger_error: Mutex<Option<String>>,
}

impl AppState {
    fn record_error(&self, error: String) {
        eprintln!("{error}");
        if let Ok(mut current) = self.error.lock() {
            *current = Some(error);
        }
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_settings(
    settings: Settings,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Settings, String> {
    let mut current = state.settings.lock().map_err(|error| error.to_string())?;
    settings.save(&state.config_path)?;
    *current = settings.clone();
    drop(current);
    window::resize(&app, state.popup_height.load(Ordering::SeqCst))?;
    Ok(settings)
}

#[tauri::command]
async fn get_status(app: tauri::AppHandle) -> Result<platform::Status, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut status = platform::status();
        if let Ok(error) = app.state::<AppState>().trigger_error.lock() {
            if let Some(error) = error.as_ref() {
                status.mouse_supported = false;
                status.error = Some(error.clone());
            }
        }
        if let Ok(error) = app.state::<AppState>().error.lock() {
            if let Some(error) = error.as_ref() {
                status.error = Some(error.clone());
            }
        }
        status
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_popup(app: tauri::AppHandle) -> Result<(), String> {
    window::hide(&app)
}

#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) -> bool {
    app.state::<AppState>().settings_view.load(Ordering::SeqCst)
}

#[tauri::command]
fn fit_popup(app: tauri::AppHandle, layout: &str) -> Result<(), String> {
    let height = match layout {
        "compact" => window::COMPACT_HEIGHT,
        "chat" => window::CHAT_HEIGHT,
        "settings" => window::SETTINGS_HEIGHT,
        _ => return Err("Invalid popup layout".into()),
    };
    app.state::<AppState>()
        .settings_view
        .store(layout == "settings", Ordering::SeqCst);
    window::resize(&app, height)
}

#[tauri::command]
async fn get_codex_connection() -> agent::ConnectionStatus {
    agent::connection_status().await
}

#[tauri::command]
async fn get_codex_models() -> Result<Vec<agent::ModelInfo>, String> {
    agent::model_list().await
}

#[tauri::command]
fn send_chat(
    request_id: String,
    text: String,
    images: Vec<agent::ImageInput>,
    app: tauri::AppHandle,
    bridge: tauri::State<'_, agent::ChatBridge>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let model = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .model
        .clone();
    bridge.send(app, request_id, text, model, images)
}

#[tauri::command]
fn stop_chat(bridge: tauri::State<'_, agent::ChatBridge>) {
    bridge.stop();
}

#[tauri::command]
async fn clear_chat(bridge: tauri::State<'_, agent::ChatBridge>) -> Result<(), String> {
    bridge.clear().await
}

pub fn run() {
    if std::env::args().any(|argument| argument == "--diagnose") {
        match serde_json::to_string_pretty(&platform::status()) {
            Ok(status) => println!("{status}"),
            Err(error) => eprintln!("{error}"),
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            |app, arguments, _cwd| {
                if let Err(error) = window::open(
                    app,
                    arguments.iter().any(|argument| argument == "--settings"),
                    arguments.iter().any(|argument| argument == "--toggle"),
                ) {
                    eprintln!("{error}");
                }
            },
        ))
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_status,
            hide_popup,
            frontend_ready,
            fit_popup,
            get_codex_connection,
            get_codex_models,
            send_chat,
            stop_chat,
            clear_chat,
            capture::capture_screen
        ])
        .setup(|app| {
            app.manage(capture::CaptureState::default());
            app.manage(agent::ChatBridge::default());
            let config_path = app.path().app_config_dir()?.join("settings.json");
            let (settings, error) = match Settings::load(&config_path) {
                Ok(settings) => (settings, None),
                Err(error) => (
                    Settings::default(),
                    Some(format!("Settings could not be loaded: {error}")),
                ),
            };
            app.manage(AppState {
                settings: Mutex::new(settings),
                config_path,
                error: Mutex::new(error),
                visible: AtomicBool::new(false),
                settings_view: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                popup_height: AtomicU32::new(window::COMPACT_HEIGHT),
                running: AtomicBool::new(true),
                trigger_error: Mutex::new(None),
            });
            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &settings_item, &quit])?;
            let mut pixels = vec![0u8; 24 * 24 * 4];
            for row in 4..20 {
                for column in 4..20 {
                    if !(8..=15).contains(&column) || row > 15 {
                        let offset = (row * 24 + column) * 4;
                        pixels[offset..offset + 4].copy_from_slice(&[224, 163, 75, 255]);
                    }
                }
            }
            TrayIconBuilder::with_id("wiggy")
                .icon(Image::new_owned(pixels, 24, 24))
                .tooltip("Wiggy")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let result = match event.id.as_ref() {
                        "open" => window::open(app, false, false),
                        "settings" => window::open(app, true, false),
                        "quit" => {
                            app.state::<agent::ChatBridge>().shutdown();
                            app.exit(0);
                            Ok(())
                        }
                        _ => Ok(()),
                    };
                    if let Err(error) = result {
                        app.state::<AppState>().record_error(error);
                    }
                })
                .build(app)?;
            #[cfg(target_os = "linux")]
            trigger::service::start(app.handle().clone());
            let arguments: Vec<String> = std::env::args().collect();
            if arguments
                .iter()
                .any(|argument| matches!(argument.as_str(), "--toggle" | "--open" | "--settings"))
            {
                window::open(
                    app.handle(),
                    arguments.iter().any(|argument| argument == "--settings"),
                    false,
                )?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window::hide(app) {
                    eprintln!("{error}");
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Wiggy failed to start")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<agent::ChatBridge>().shutdown();
                app.state::<AppState>()
                    .running
                    .store(false, Ordering::SeqCst);
            }
        });
}
