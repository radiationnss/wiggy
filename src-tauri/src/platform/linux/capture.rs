use super::{DesktopMonitor, Hyprland};
use crate::capture::{Screenshot, Target};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

const IMAGE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Client {
    pid: u32,
    at: [i32; 2],
    size: [u32; 2],
    workspace: Workspace,
    #[serde(default)]
    mapped: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    pinned: bool,
}

#[derive(Deserialize)]
struct Workspace {
    id: i64,
}

fn window_boxes(clients: &[Client], monitors: &[DesktopMonitor], own_pid: u32) -> String {
    clients
        .iter()
        .filter(|client| {
            client.pid != own_pid
                && client.mapped
                && !client.hidden
                && client.size[0] > 0
                && client.size[1] > 0
                && client.size[0] <= 32768
                && client.size[1] <= 32768
                && monitors.iter().any(|monitor| {
                    !monitor.geometry.disabled
                        && (client.pinned
                            || client.workspace.id == monitor.active_workspace.id
                            || (monitor.special_workspace.id != 0
                                && client.workspace.id == monitor.special_workspace.id))
                })
        })
        .take(512)
        .map(|client| {
            format!(
                "{},{} {}x{}\n",
                client.at[0], client.at[1], client.size[0], client.size[1]
            )
        })
        .collect()
}

fn geometry(value: &str) -> Result<String, String> {
    let parts: Vec<_> = value.split_whitespace().collect();
    if parts.len() != 2 {
        return Err("Invalid capture selection".into());
    }
    let (horizontal, vertical) = parts[0]
        .split_once(',')
        .ok_or("Invalid capture coordinates")?;
    let (width, height) = parts[1]
        .split_once('x')
        .ok_or("Invalid capture dimensions")?;
    let horizontal = horizontal
        .parse::<i32>()
        .map_err(|_| "Invalid capture coordinates")?;
    let vertical = vertical
        .parse::<i32>()
        .map_err(|_| "Invalid capture coordinates")?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "Invalid capture dimensions")?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "Invalid capture dimensions")?;
    if width == 0 || height == 0 || width > 32768 || height > 32768 {
        return Err("Capture area is empty or too large".into());
    }
    horizontal
        .checked_add(i32::try_from(width).map_err(|_| "Invalid capture width")?)
        .ok_or("Capture coordinates overflow")?;
    vertical
        .checked_add(i32::try_from(height).map_err(|_| "Invalid capture height")?)
        .ok_or("Capture coordinates overflow")?;
    Ok(format!("{horizontal},{vertical} {width}x{height}"))
}

#[derive(Debug)]
struct Output {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn run(command: &mut Command, input: Option<&str>, limit: usize) -> Result<Output, String> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        format!("Could not start capture tool: {error}. Install grim and slurp.")
    })?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or("Missing capture output")?
        .take((limit + 1) as u64);
    let mut stderr = child
        .stderr
        .take()
        .ok_or("Missing capture error output")?
        .take(8193);
    let mut stdin = child.stdin.take();
    let operation = async {
        let write = async {
            if let (Some(mut pipe), Some(input)) = (stdin.take(), input) {
                pipe.write_all(input.as_bytes()).await?;
            }
            Ok::<_, std::io::Error>(())
        };
        let mut bytes = Vec::new();
        let mut errors = Vec::new();
        let read = async {
            stdout.read_to_end(&mut bytes).await?;
            if bytes.len() > limit {
                return Err(std::io::Error::other(
                    "Capture exceeds size limit; select a smaller area",
                ));
            }
            Ok::<_, std::io::Error>(())
        };
        let read_errors = async {
            stderr.read_to_end(&mut errors).await?;
            if errors.len() > 8192 {
                return Err(std::io::Error::other(
                    "Capture tool error output exceeds limit",
                ));
            }
            Ok::<_, std::io::Error>(())
        };
        let (_, _, _, status) = tokio::try_join!(write, read, read_errors, child.wait())
            .map_err(|error| error.to_string())?;
        Ok(Output {
            status,
            stdout: bytes,
            stderr: errors,
        })
    };
    tokio::time::timeout(Duration::from_secs(60), operation)
        .await
        .map_err(|_| "Screen capture timed out; try again".to_string())?
}

fn selection(output: Output) -> Result<Option<String>, String> {
    let error = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if output.status.code() == Some(1) && error.trim() == "selection cancelled" {
            return Ok(None);
        }
        return Err(format!(
            "Screen selection failed: {}",
            if error.trim().is_empty() {
                "selector exited unexpectedly"
            } else {
                error.trim()
            }
        ));
    }
    let value = std::str::from_utf8(&output.stdout).map_err(|_| "Invalid capture selection")?;
    geometry(value).map(Some)
}

fn screenshot(bytes: Vec<u8>, target: Target) -> Result<Screenshot, String> {
    if bytes.len() < 33
        || bytes.len() > IMAGE_LIMIT
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
    {
        return Err("Capture did not produce a valid bounded PNG".into());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| "Invalid PNG width")?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| "Invalid PNG height")?);
    if width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || u64::from(width) * u64::from(height) > 32_000_000
    {
        return Err("Capture dimensions exceed limit; select a smaller area".into());
    }
    Ok(Screenshot {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(&bytes)),
        width,
        height,
        bytes: bytes.len(),
        label: match target {
            Target::Screen => "Entire screen",
            Target::Region => "Selected area",
            Target::Window => "Window",
        },
    })
}

pub async fn capture(target: Target) -> Result<Option<Screenshot>, String> {
    let adapter = Hyprland::connect()?;
    let version = adapter.version()?;
    if !Hyprland::supported(&version) {
        return Err(format!(
            "Screen capture requires verified Hyprland 0.55/0.56, found {version}"
        ));
    }
    tokio::time::sleep(Duration::from_millis(160)).await;
    let selected = match target {
        Target::Screen => None,
        Target::Region | Target::Window => {
            let boxes = if matches!(target, Target::Window) {
                let clients = serde_json::from_str::<Vec<Client>>(&adapter.request("j/clients")?)
                    .map_err(|error| error.to_string())?;
                let monitors =
                    serde_json::from_str::<Vec<DesktopMonitor>>(&adapter.request("j/monitors")?)
                        .map_err(|error| error.to_string())?;
                let boxes = window_boxes(&clients, &monitors, std::process::id());
                if boxes.is_empty() {
                    return Err("No visible windows to capture".into());
                }
                Some(boxes)
            } else {
                None
            };
            let mut selector = Command::new("/usr/bin/slurp");
            selector.args(["-f", "%x,%y %wx%h"]);
            if boxes.is_some() {
                selector.arg("-r");
            }
            let Some(selected) = selection(run(&mut selector, boxes.as_deref(), 4096).await?)?
            else {
                return Ok(None);
            };
            tokio::time::sleep(Duration::from_millis(100)).await;
            Some(selected)
        }
    };
    let mut command = Command::new("/usr/bin/grim");
    command.args(["-t", "png", "-s", "1"]);
    if let Some(selected) = selected {
        command.args(["-g", &selected]);
    }
    command.arg("-");
    let output = run(&mut command, None, IMAGE_LIMIT).await?;
    if !output.status.success() {
        return Err(format!(
            "Screen capture failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    screenshot(output.stdout, target).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_selection_without_shell_interpolation() {
        assert_eq!(geometry("-1920,12 640x480\n").unwrap(), "-1920,12 640x480");
        for value in [
            "0,0 0x1",
            "0,0 4x-2",
            "1,2 2x3;touch",
            "0,0 1x1 extra",
            "2147483647,0 2x2",
        ] {
            assert!(geometry(value).is_err(), "{value}");
        }
    }

    #[test]
    fn filters_hidden_other_workspace_and_own_windows() {
        let clients: Vec<Client> = serde_json::from_value(serde_json::json!([
            {"pid":1,"at":[-500,0],"size":[300,200],"workspace":{"id":2},"mapped":true},
            {"pid":2,"at":[0,0],"size":[300,200],"workspace":{"id":1},"mapped":true},
            {"pid":3,"at":[0,0],"size":[300,200],"workspace":{"id":2},"mapped":true,"hidden":true},
            {"pid":4,"at":[0,0],"size":[300,200],"workspace":{"id":2},"mapped":true},
            {"pid":5,"at":[0,0],"size":[300,200],"workspace":{"id":9},"mapped":true,"pinned":true}
        ]))
        .unwrap();
        let monitors: Vec<DesktopMonitor> = serde_json::from_value(serde_json::json!([
            {"name":"DP-1","x":-1920,"y":0,"width":1920,"height":1080,"scale":1,"activeWorkspace":{"id":2,"name":"2"},"specialWorkspace":{"id":0,"name":""}}
        ])).unwrap();
        assert_eq!(
            window_boxes(&clients, &monitors, 4),
            "-500,0 300x200\n0,0 300x200\n"
        );
    }

    #[test]
    fn rejects_invalid_or_oversized_images() {
        assert!(screenshot(vec![], Target::Screen).is_err());
        let mut bytes = vec![0; 33];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&480u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&320u32.to_be_bytes());
        let result = screenshot(bytes.clone(), Target::Region).unwrap();
        assert_eq!(
            (result.width, result.height, result.label),
            (480, 320, "Selected area")
        );
        assert!(result.data_url.starts_with("data:image/png;base64,"));
        bytes[16..20].copy_from_slice(&20000u32.to_be_bytes());
        assert!(screenshot(bytes, Target::Window).is_err());
    }

    #[test]
    fn cancellation_is_distinct_from_connection_failure() {
        use std::os::unix::process::ExitStatusExt;
        let cancelled = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: b"selection cancelled\n".to_vec(),
        };
        assert!(selection(cancelled).unwrap().is_none());
        let failed = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: b"failed to connect to display".to_vec(),
        };
        assert!(selection(failed).is_err());
    }

    #[test]
    fn subprocess_output_is_bounded() {
        tauri::async_runtime::block_on(async {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", "printf 123456789"]);
            assert!(run(&mut command, None, 4)
                .await
                .unwrap_err()
                .contains("size limit"));
        });
    }

    #[test]
    fn selector_input_is_closed_after_boxes_are_written() {
        tauri::async_runtime::block_on(async {
            let mut command = Command::new("/bin/cat");
            let output = run(&mut command, Some("10,20 300x200\n"), 4096)
                .await
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"10,20 300x200\n");
        });
    }

    #[test]
    #[ignore = "Requires an explicitly approved live Hyprland screen capture"]
    fn captures_real_desktop_in_memory() {
        tauri::async_runtime::block_on(async {
            let image = capture(Target::Screen).await.unwrap().unwrap();
            assert!(image.width > 0 && image.height > 0 && image.bytes > 33);
            assert!(image.data_url.starts_with("data:image/png;base64,"));
            println!(
                "Desktop capture: {}x{}, {} bytes; image stays in memory",
                image.width, image.height, image.bytes
            );
        });
    }
}
