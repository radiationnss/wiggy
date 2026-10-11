use super::WiggleDetector;
use crate::{
    config::Settings, platform::linux::Hyprland, platform::CursorSource, window, AppState,
};
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

#[derive(Default)]
struct Tracking {
    detector: WiggleDetector,
    settings: Option<Settings>,
}

impl Tracking {
    fn sample(
        &mut self,
        source: &impl CursorSource,
        settings: &Settings,
        paused: bool,
        time: Duration,
    ) -> Result<bool, String> {
        if self.settings.as_ref() != Some(settings) {
            self.detector.reset();
            self.settings = Some(settings.clone());
        }
        if !settings.mouse_enabled || paused {
            self.detector.reset();
            return Ok(false);
        }
        match source.cursor() {
            Ok(point) => Ok(self.detector.sample(
                point,
                time,
                settings.sensitivity,
                Duration::from_millis(settings.cooldown_ms),
            )),
            Err(error) => {
                self.detector.reset();
                Err(error)
            }
        }
    }
}

fn report(app: &tauri::AppHandle, error: Option<String>) {
    let state = app.state::<AppState>();
    let changed = if let Ok(mut current) = state.trigger_error.lock() {
        if *current != error {
            *current = error;
            true
        } else {
            false
        }
    } else {
        false
    };
    if changed {
        let _ = app.emit("trigger-status-changed", ());
    }
}

pub(crate) fn start(app: tauri::AppHandle) {
    thread::spawn(move || {
        let started = Instant::now();
        let mut tracking = Tracking::default();
        let mut source = None;
        let mut retry_at = Instant::now();
        while app.state::<AppState>().running.load(Ordering::SeqCst) {
            let tick = Instant::now();
            let state = app.state::<AppState>();
            let settings = match state.settings.lock() {
                Ok(settings) => settings.clone(),
                Err(_) => break,
            };
            if !settings.mouse_enabled {
                tracking.detector.reset();
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            if source.is_none() && tick >= retry_at {
                let connect = || {
                    let adapter = Hyprland::connect()?;
                    let version = adapter.version()?;
                    if !Hyprland::supported(&version) {
                        return Err(format!(
                            "Mouse trigger: unverified Hyprland version {version}"
                        ));
                    }
                    Ok(adapter)
                };
                match connect() {
                    Ok(adapter) => source = Some(adapter),
                    Err(error) => {
                        report(&app, Some(error));
                        retry_at = tick + Duration::from_secs(2);
                    }
                }
            }
            if let Some(adapter) = source.as_ref() {
                let paused = app
                    .state::<crate::capture::CaptureState>()
                    .busy
                    .load(Ordering::SeqCst);
                match tracking.sample(adapter, &settings, paused, started.elapsed()) {
                    Ok(triggered) => {
                        report(&app, None);
                        if triggered {
                            let handle = app.clone();
                            if let Err(error) = app.run_on_main_thread(move || {
                                let state = handle.state::<AppState>();
                                let allowed = state
                                    .settings
                                    .lock()
                                    .is_ok_and(|settings| settings.mouse_enabled);
                                if allowed {
                                    if let Err(error) = window::open(&handle, false, false) {
                                        state.record_error(error);
                                    }
                                }
                            }) {
                                report(&app, Some(error.to_string()));
                            }
                        }
                    }
                    Err(error) => {
                        report(&app, Some(format!("Mouse trigger: {error}")));
                        source = None;
                        retry_at = tick + Duration::from_secs(2);
                    }
                }
            }
            thread::sleep(Duration::from_millis(33).saturating_sub(tick.elapsed()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::geometry::Point;
    use std::cell::{Cell, RefCell};

    struct Cursor {
        point: RefCell<Result<Point, String>>,
        reads: Cell<usize>,
    }

    impl CursorSource for Cursor {
        fn cursor(&self) -> Result<Point, String> {
            self.reads.set(self.reads.get() + 1);
            self.point.borrow().clone()
        }
    }

    fn cursor() -> Cursor {
        Cursor {
            point: RefCell::new(Ok(Point { x: 0.0, y: 0.0 })),
            reads: Cell::new(0),
        }
    }

    fn stroke(
        tracking: &mut Tracking,
        source: &Cursor,
        settings: &Settings,
        time: u64,
        horizontal: f64,
    ) -> bool {
        *source.point.borrow_mut() = Ok(Point {
            x: horizontal,
            y: 0.0,
        });
        tracking
            .sample(source, settings, false, Duration::from_millis(time))
            .unwrap()
    }

    #[test]
    fn adapter_samples_trigger_and_respect_cooldown() {
        let source = cursor();
        let settings = Settings::default();
        let mut tracking = Tracking::default();
        let mut triggers = 0;
        for base in [0, 500, 3000] {
            for (index, horizontal) in [0.0, 60.0, 0.0, 60.0, 0.0].into_iter().enumerate() {
                triggers += usize::from(stroke(
                    &mut tracking,
                    &source,
                    &settings,
                    base + index as u64 * 50,
                    horizontal,
                ));
            }
        }
        assert_eq!(triggers, 2);
        assert_eq!(source.reads.get(), 15);
    }

    #[test]
    fn vertical_then_diagonal_gestures_share_one_cooldown() {
        let source = cursor();
        let settings = Settings::default();
        let mut tracking = Tracking::default();
        let mut triggers = 0;
        for (base, degrees) in [(0, 90), (500, 45), (3000, 135)] {
            let angle = f64::from(degrees).to_radians();
            for (index, distance) in [0.0, 60.0, 0.0, 60.0, 0.0].into_iter().enumerate() {
                *source.point.borrow_mut() = Ok(Point {
                    x: distance * angle.cos(),
                    y: distance * angle.sin(),
                });
                triggers += usize::from(
                    tracking
                        .sample(
                            &source,
                            &settings,
                            false,
                            Duration::from_millis(base + index as u64 * 50),
                        )
                        .unwrap(),
                );
            }
        }
        assert_eq!(triggers, 2);
    }

    #[test]
    fn paused_or_disabled_does_not_read_cursor_and_resets_gesture() {
        let source = cursor();
        let mut settings = Settings::default();
        let mut tracking = Tracking::default();
        for (index, horizontal) in [0.0, 60.0, 0.0, 60.0].into_iter().enumerate() {
            assert!(!stroke(
                &mut tracking,
                &source,
                &settings,
                index as u64 * 50,
                horizontal
            ));
        }
        assert!(!tracking
            .sample(&source, &settings, true, Duration::from_millis(175))
            .unwrap());
        assert_eq!(source.reads.get(), 4);
        assert!(!stroke(&mut tracking, &source, &settings, 200, 0.0));
        settings.mouse_enabled = false;
        assert!(!tracking
            .sample(&source, &settings, false, Duration::from_millis(250))
            .unwrap());
        assert_eq!(source.reads.get(), 5);
    }

    #[test]
    fn every_position_mode_can_be_recalled_and_disabled_tracking_still_blocks_it() {
        use crate::config::PopupPosition;
        for popup_position in [
            PopupPosition::NearCursor,
            PopupPosition::TopLeft,
            PopupPosition::TopRight,
            PopupPosition::BottomLeft,
            PopupPosition::BottomRight,
        ] {
            for mouse_enabled in [false, true] {
                let settings = Settings {
                    popup_position,
                    mouse_enabled,
                    ..Settings::default()
                };
                let source = cursor();
                let mut tracking = Tracking::default();
                let mut triggers = 0;
                for (index, horizontal) in [0.0, 60.0, 0.0, 60.0, 0.0].into_iter().enumerate() {
                    triggers += usize::from(stroke(
                        &mut tracking,
                        &source,
                        &settings,
                        index as u64 * 50,
                        horizontal,
                    ));
                }
                assert_eq!(triggers, usize::from(mouse_enabled), "{popup_position:?}");
                assert_eq!(source.reads.get(), if mouse_enabled { 5 } else { 0 });
            }
        }
    }

    #[test]
    fn ipc_failure_and_settings_changes_discard_partial_gestures() {
        let source = cursor();
        let mut settings = Settings::default();
        let mut tracking = Tracking::default();
        for (index, horizontal) in [0.0, 60.0, 0.0, 60.0].into_iter().enumerate() {
            assert!(!stroke(
                &mut tracking,
                &source,
                &settings,
                index as u64 * 50,
                horizontal
            ));
        }
        *source.point.borrow_mut() = Err("disconnected".into());
        assert!(tracking
            .sample(&source, &settings, false, Duration::from_millis(175))
            .is_err());
        assert!(!stroke(&mut tracking, &source, &settings, 200, 0.0));
        assert!(!stroke(&mut tracking, &source, &settings, 250, 60.0));
        assert!(!stroke(&mut tracking, &source, &settings, 300, 0.0));
        assert!(!stroke(&mut tracking, &source, &settings, 350, 60.0));
        settings.sensitivity = 2.0;
        assert!(!stroke(&mut tracking, &source, &settings, 400, 0.0));
    }
}
