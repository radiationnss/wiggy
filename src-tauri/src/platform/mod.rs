use crate::window::geometry::Point;
use serde::Serialize;

#[cfg(target_os = "linux")]
pub mod linux;

pub trait CursorSource {
    fn cursor(&self) -> Result<Point, String>;
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub platform: String,
    pub version: Option<String>,
    pub mouse_supported: bool,
    pub position_supported: bool,
    pub error: Option<String>,
    pub instructions: String,
}

pub fn status() -> Status {
    #[cfg(target_os = "linux")]
    {
        linux::Hyprland::status()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Status {
            platform: std::env::consts::OS.into(),
            version: None,
            mouse_supported: false,
            position_supported: false,
            error: Some(
                "Native tracking and global shortcuts are not implemented on this platform".into(),
            ),
            instructions: "Use the tray to open the palette.".into(),
        }
    }
}
