use super::{CursorSource, Status};
pub mod capture;
use crate::config::PopupPosition;
use crate::window::geometry::{Monitor, Point};
use serde::Deserialize;
use std::{
    env,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

#[derive(Clone)]
pub struct Hyprland {
    socket: PathBuf,
}

#[derive(Deserialize)]
struct Version {
    version: String,
}

#[derive(Deserialize)]
struct Client {
    address: String,
    pid: u32,
    #[serde(rename = "initialTitle")]
    initial_title: String,
    at: [f64; 2],
}

#[derive(Deserialize)]
struct Workspace {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
struct DesktopMonitor {
    #[serde(flatten)]
    geometry: Monitor,
    #[serde(rename = "activeWorkspace")]
    active_workspace: Workspace,
    #[serde(rename = "specialWorkspace")]
    special_workspace: Workspace,
}

impl DesktopMonitor {
    fn workspace_selector(&self) -> Result<String, String> {
        let workspace = if self.special_workspace.id != 0 {
            &self.special_workspace
        } else {
            &self.active_workspace
        };
        if workspace.id > 0 {
            Ok(workspace.id.to_string())
        } else if workspace.name.starts_with("special:") {
            serde_json::to_string(&workspace.name).map_err(|error| error.to_string())
        } else if !workspace.name.is_empty() {
            serde_json::to_string(&format!("name:{}", workspace.name))
                .map_err(|error| error.to_string())
        } else {
            Err(format!(
                "Monitor {} has no active workspace",
                self.geometry.name
            ))
        }
    }
}

fn workspace_dispatch(selector: &str, workspace: &str) -> String {
    format!("hl.dsp.window.move({{{selector}, workspace = {workspace}, follow = false}})")
}

fn floating_dispatch(selector: &str) -> String {
    format!("hl.dsp.window.float({{{selector}, action = \"enable\"}})")
}

impl Hyprland {
    pub fn connect() -> Result<Self, String> {
        let runtime = env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unset")?;
        let signature =
            env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|_| "Not a Hyprland session")?;
        if signature.is_empty() || signature.contains('/') || signature.contains("..") {
            return Err("Invalid Hyprland instance signature".into());
        }
        Ok(Self {
            socket: PathBuf::from(runtime)
                .join("hypr")
                .join(signature)
                .join(".socket.sock"),
        })
    }

    pub fn request(&self, command: &str) -> Result<String, String> {
        let mut stream =
            UnixStream::connect(&self.socket).map_err(|error| format!("Hyprland IPC: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(250)))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_millis(250)))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(command.as_bytes())
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        stream
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > 1_048_576 {
            return Err("Hyprland response exceeds limit".into());
        }
        String::from_utf8(bytes).map_err(|error| error.to_string())
    }

    pub fn version(&self) -> Result<String, String> {
        serde_json::from_str::<Version>(&self.request("j/version")?)
            .map(|value| value.version)
            .map_err(|error| error.to_string())
    }

    pub fn supported(version: &str) -> bool {
        let parts: Vec<_> = version.trim_start_matches('v').split('.').collect();
        parts.len() == 3
            && parts[0] == "0"
            && matches!(parts[1], "55" | "56")
            && parts[2].parse::<u32>().is_ok()
    }

    pub fn status() -> Status {
        let mut result = Status {
            platform: "Hyprland".into(),
            version: None,
            mouse_supported: false,
            position_supported: false,
            error: None,
            instructions: "Unsupported compositor/version. Use the tray; no rules were installed."
                .into(),
        };
        let probe = || -> Result<String, String> {
            let adapter = Self::connect()?;
            let version = adapter.version()?;
            adapter.cursor()?;
            Ok(version)
        };
        match probe() {
            Ok(version) => {
                if Self::supported(&version) {
                    result.mouse_supported = true;
                    result.position_supported = true;
                    result.instructions = "hl.window_rule({ match = { class = \"wiggy\" }, float = true, size = {480, 180} })\nhl.bind(\"SUPER + SHIFT + Space\", hl.dsp.exec_cmd(\"/absolute/path/to/wiggy --toggle\"))".into();
                } else {
                    result.error = Some("Only Hyprland 0.55/0.56 Lua dispatches are supported; older/newer versions need verification".into());
                }
                result.version = Some(version);
            }
            Err(error) => {
                result.error = Some(error);
            }
        }
        result
    }

    fn dispatch(&self, expression: &str) -> Result<(), String> {
        let reply = self.request(&format!("dispatch {expression}"))?;
        if reply.trim() != "ok" {
            return Err(format!("Hyprland dispatch: {reply}"));
        }
        Ok(())
    }

    pub fn place_popup(
        &self,
        position: PopupPosition,
        height: u32,
        opening: bool,
    ) -> Result<(), String> {
        let version = self.version()?;
        if !Self::supported(&version) {
            return Err(format!("Unverified Hyprland window API: {version}"));
        }
        let clients: Vec<Client> =
            serde_json::from_str(&self.request("j/clients")?).map_err(|error| error.to_string())?;
        let client = clients
            .iter()
            .find(|client| client.pid == std::process::id() && client.initial_title == "Wiggy")
            .ok_or("Wiggy window has not mapped yet")?;
        let address = &client.address;
        if !address.starts_with("0x")
            || address.len() <= 2
            || !address[2..]
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err("Invalid window address".into());
        }
        let cursor = self.cursor()?;
        let monitors: Vec<DesktopMonitor> = serde_json::from_str(&self.request("j/monitors")?)
            .map_err(|error| error.to_string())?;
        let location = if opening {
            cursor
        } else {
            Point {
                x: client.at[0],
                y: client.at[1],
            }
        };
        let monitor = monitors
            .iter()
            .find(|monitor| monitor.geometry.contains(location))
            .or_else(|| monitors.iter().find(|monitor| monitor.geometry.focused))
            .ok_or("No usable monitor")?;
        let placement = monitor.geometry.place(
            if opening {
                cursor
            } else {
                Point {
                    x: location.x - 18.0,
                    y: location.y - 18.0,
                }
            },
            position,
            height,
        )?;
        let selector = format!("window = \"address:{address}\"");
        self.dispatch(&floating_dispatch(&selector))?;
        if opening {
            self.dispatch(&format!(
                "hl.dsp.window.pin({{{selector}, action = \"disable\"}})"
            ))?;
            self.dispatch(&workspace_dispatch(
                &selector,
                &monitor.workspace_selector()?,
            ))?;
        }
        self.dispatch(&format!(
            "hl.dsp.window.resize({{{selector}, x = {}, y = {}, relative = false}})",
            placement.width, placement.height
        ))?;
        self.dispatch(&format!(
            "hl.dsp.window.move({{{selector}, x = {}, y = {}, relative = false}})",
            placement.x, placement.y
        ))?;
        if opening {
            self.dispatch(&format!("hl.dsp.focus({{{selector}}})"))?;
        }
        Ok(())
    }
}

impl CursorSource for Hyprland {
    fn cursor(&self) -> Result<Point, String> {
        let point: Point = serde_json::from_str(&self.request("j/cursorpos")?)
            .map_err(|error| error.to_string())?;
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err("Non-finite cursor position".into());
        }
        Ok(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(name: &str, horizontal: f64, workspace: i64) -> serde_json::Value {
        serde_json::json!({
            "name": name, "x": horizontal, "y": 0, "width": 1920,
            "height": 1080, "scale": 1, "reserved": [0, 39, 0, 0],
            "focused": horizontal == 0.0,
            "activeWorkspace": {"id": workspace, "name": workspace.to_string()},
            "specialWorkspace": {"id": 0, "name": ""}
        })
    }

    #[test]
    fn selects_regular_named_and_special_workspaces_without_lua_injection() {
        let mut value = monitor("test", 0.0, 4);
        let regular: DesktopMonitor = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(regular.workspace_selector().unwrap(), "4");
        value["activeWorkspace"] = serde_json::json!({"id": -1337, "name": "work\"desk"});
        let named: DesktopMonitor = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(named.workspace_selector().unwrap(), "\"name:work\\\"desk\"");
        value["specialWorkspace"] = serde_json::json!({"id": -99, "name": "special:scratch"});
        let special: DesktopMonitor = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(special.workspace_selector().unwrap(), "\"special:scratch\"");
        value["specialWorkspace"] = serde_json::json!({"id": 0, "name": ""});
        value["activeWorkspace"] = serde_json::json!({"id": 0, "name": ""});
        let invalid: DesktopMonitor = serde_json::from_value(value).unwrap();
        assert!(invalid.workspace_selector().is_err());
    }

    fn placement_requests(opening: bool, position: PopupPosition) -> Vec<String> {
        use std::{os::unix::net::UnixListener, thread, time::Instant};
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let socket =
            env::temp_dir().join(format!("wiggy-ipc-{}-{unique}.sock", std::process::id()));
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut requests = Vec::new();
            let expected = if opening { 10 } else { 7 };
            while requests.len() < expected && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(250)))
                    .unwrap();
                let mut bytes = [0; 4096];
                let count = stream.read(&mut bytes).unwrap();
                let command = String::from_utf8(bytes[..count].to_vec()).unwrap();
                let reply = match command.as_str() {
                    "j/version" => serde_json::json!({"version": "0.56.2"}).to_string(),
                    "j/clients" => serde_json::json!([{
                        "address": "0x123", "pid": std::process::id(),
                        "initialTitle": "Wiggy", "at": [100, 100]
                    }])
                    .to_string(),
                    "j/cursorpos" => serde_json::json!({"x": 2100, "y": 500}).to_string(),
                    "j/monitors" => {
                        serde_json::json!([monitor("left", 0.0, 1), monitor("right", 1920.0, 7)])
                            .to_string()
                    }
                    _ => "ok".into(),
                };
                stream.write_all(reply.as_bytes()).unwrap();
                requests.push(command);
            }
            requests
        });
        let result = Hyprland {
            socket: socket.clone(),
        }
        .place_popup(position, 180, opening);
        let requests = server.join().unwrap();
        std::fs::remove_file(socket).unwrap();
        result.unwrap();
        requests
    }

    #[test]
    fn opening_summons_existing_window_to_cursor_monitors_workspace_without_pinning() {
        let requests = placement_requests(true, PopupPosition::NearCursor);
        assert_eq!(requests.len(), 10);
        assert!(requests.contains(
            &"dispatch hl.dsp.window.pin({window = \"address:0x123\", action = \"disable\"})"
                .into()
        ));
        assert!(requests.contains(&"dispatch hl.dsp.window.move({window = \"address:0x123\", workspace = 7, follow = false})".into()));
        assert!(requests.contains(&"dispatch hl.dsp.window.move({window = \"address:0x123\", x = 2118, y = 518, relative = false})".into()));
        assert!(requests
            .last()
            .unwrap()
            .starts_with("dispatch hl.dsp.focus("));
    }

    #[test]
    fn resizing_keeps_current_monitor_workspace_and_position_without_focus_theft() {
        let requests = placement_requests(false, PopupPosition::NearCursor);
        assert_eq!(requests.len(), 7);
        assert!(!requests
            .iter()
            .any(|request| request.contains("workspace =")
                || request.contains(".pin(")
                || request.contains("hl.dsp.focus(")));
        assert!(requests.contains(&"dispatch hl.dsp.window.move({window = \"address:0x123\", x = 100, y = 100, relative = false})".into()));
    }

    #[test]
    fn corner_recall_uses_selected_corner_of_cursor_monitor_and_current_workspace() {
        for (position, horizontal, vertical) in [
            (PopupPosition::TopLeft, 1928, 47),
            (PopupPosition::TopRight, 3352, 47),
            (PopupPosition::BottomLeft, 1928, 892),
            (PopupPosition::BottomRight, 3352, 892),
        ] {
            let requests = placement_requests(true, position);
            assert!(requests.contains(&"dispatch hl.dsp.window.move({window = \"address:0x123\", workspace = 7, follow = false})".into()));
            assert!(requests.contains(&format!("dispatch hl.dsp.window.move({{window = \"address:0x123\", x = {horizontal}, y = {vertical}, relative = false}})")), "{position:?}");
            assert!(!requests
                .iter()
                .any(|request| request.contains(".pin(") && request.contains("enable")));
        }
    }

    #[test]
    fn floating_request_is_idempotent_not_a_toggle() {
        assert_eq!(
            floating_dispatch("window = \"address:0x123\""),
            "hl.dsp.window.float({window = \"address:0x123\", action = \"enable\"})"
        );
    }
    #[test]
    fn refuses_unverified_versions() {
        assert!(Hyprland::supported("0.56.2"));
        assert!(Hyprland::supported("v0.55.0"));
        for version in ["0.53.2", "0.57.0", "unknown", "0.56.2-dev"] {
            assert!(!Hyprland::supported(version));
        }
    }
}
