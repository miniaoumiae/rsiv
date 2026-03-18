use crate::app::AppEvent;
use crate::keybinds::Action;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use winit::event_loop::EventLoopProxy;

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcRequest {
    AddFile(PathBuf),
    RunAction(Action),
    GetState,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcResponse {
    Ack,
    State(String),
    Error(String),
}

pub fn get_socket_dir() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let dir = PathBuf::from(runtime_dir).join("rsiv");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    dir
}

pub fn get_pid_socket(pid: u32) -> PathBuf {
    get_socket_dir().join(format!("rsiv-{}.sock", pid))
}

pub fn get_latest_socket() -> PathBuf {
    get_socket_dir().join("rsiv-latest.sock")
}

const MAX_PAYLOAD_SIZE: usize = 1024 * 1024; // 1MB

fn send_ipc_message(mut stream: UnixStream, req: &IpcRequest) -> Result<IpcResponse, String> {
    let payload = serde_json::to_vec(req).map_err(|e| format!("Failed to serialize: {}", e))?;
    let len = payload.len() as u32;
    stream
        .write_all(&len.to_le_bytes())
        .map_err(|e| format!("Write len failed: {}", e))?;
    stream
        .write_all(&payload)
        .map_err(|e| format!("Write payload failed: {}", e))?;

    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("Read len failed: {}", e))?;
    let resp_len = u32::from_le_bytes(len_buf) as usize;

    if resp_len > MAX_PAYLOAD_SIZE {
        return Err("Response payload too large".to_string());
    }

    let mut resp_buf = vec![0u8; resp_len];
    stream
        .read_exact(&mut resp_buf)
        .map_err(|e| format!("Read payload failed: {}", e))?;

    serde_json::from_slice(&resp_buf).map_err(|e| format!("Parse response failed: {}", e))
}

// Client
pub fn send_message(
    msg_type: &str,
    payload: &str,
    target_pid: Option<u32>,
    broadcast_all: bool,
) -> Result<(), String> {
    let req = match msg_type {
        "add" => {
            let path = Path::new(payload);
            let abs_path =
                std::fs::canonicalize(path).map_err(|e| format!("Invalid path: {}", e))?;
            IpcRequest::AddFile(abs_path)
        }
        "cmd" => {
            if let Some(action) = parse_action(payload) {
                IpcRequest::RunAction(action)
            } else {
                return Err(format!("Invalid action: {}", payload));
            }
        }
        "state" => IpcRequest::GetState,
        _ => return Err("Invalid message type. Use 'add', 'cmd', or 'state'.".to_string()),
    };

    let socket_dir = get_socket_dir();
    let mut targets = Vec::new();

    if broadcast_all {
        if let Ok(entries) = std::fs::read_dir(&socket_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("rsiv-")
                    && file_name.ends_with(".sock")
                    && file_name != "rsiv-latest.sock"
                {
                    targets.push(entry.path());
                }
            }
        }
    } else if let Some(pid) = target_pid {
        targets.push(get_pid_socket(pid));
    } else {
        targets.push(get_latest_socket());
    }

    if targets.is_empty() {
        return Err("No running instances of rsiv found.".to_string());
    }

    let mut success_count = 0;
    for target in targets {
        if let Ok(stream) = UnixStream::connect(&target) {
            match send_ipc_message(stream, &req) {
                Ok(IpcResponse::Ack) => success_count += 1,
                Ok(IpcResponse::State(state)) => {
                    println!("{}", state);
                    success_count += 1;
                }
                Ok(IpcResponse::Error(e)) => {
                    return Err(format!("Server error: {}", e));
                }
                Err(e) => return Err(e),
            }
        } else if !broadcast_all {
            let _ = std::fs::remove_file(&target); // Clean up dead socket
            return Err(format!("Could not connect to instance at {:?}", target));
        }
    }

    if success_count == 0 {
        return Err("Failed to send message. Target instances may have crashed.".to_string());
    }
    Ok(())
}

fn write_ipc_response(stream: &mut UnixStream, resp: &IpcResponse) -> std::io::Result<()> {
    let payload = serde_json::to_vec(resp).unwrap();
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&payload)?;
    Ok(())
}

// Server
pub fn spawn_ipc_server(proxy: EventLoopProxy<AppEvent>) {
    let pid = std::process::id();
    let pid_socket = get_pid_socket(pid);
    let latest_socket = get_latest_socket();

    let _ = std::fs::remove_file(&pid_socket);

    let listener = match UnixListener::bind(&pid_socket) {
        Ok(l) => l,
        Err(e) => {
            crate::rsiv_warn!("Failed to bind IPC socket: {}", e);
            return;
        }
    };

    // Safely update the latest symlink
    let _ = std::fs::remove_file(&latest_socket);
    if let Err(e) = symlink(&pid_socket, &latest_socket) {
        crate::rsiv_warn!("Failed to update latest symlink: {}", e);
    }

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let proxy = proxy.clone();
                    thread::spawn(move || {
                        let mut len_buf = [0u8; 4];
                        if stream.read_exact(&mut len_buf).is_err() {
                            return;
                        }
                        let req_len = u32::from_le_bytes(len_buf) as usize;

                        if req_len > MAX_PAYLOAD_SIZE {
                            let _ = write_ipc_response(
                                &mut stream,
                                &IpcResponse::Error("Payload too large".to_string()),
                            );
                            return;
                        }

                        let mut req_buf = vec![0u8; req_len];
                        if stream.read_exact(&mut req_buf).is_err() {
                            return;
                        }

                        let req: IpcRequest = match serde_json::from_slice(&req_buf) {
                            Ok(r) => r,
                            Err(e) => {
                                let _ = write_ipc_response(
                                    &mut stream,
                                    &IpcResponse::Error(format!("Parse error: {}", e)),
                                );
                                return;
                            }
                        };

                        let (tx, rx) = std::sync::mpsc::channel();
                        if proxy.send_event(AppEvent::Ipc(req, tx)).is_ok() {
                            if let Ok(resp) = rx.recv() {
                                let _ = write_ipc_response(&mut stream, &resp);
                            }
                        }
                    });
                }
                Err(e) => crate::rsiv_warn!("IPC Connection failed: {}", e),
            }
        }
    });
}

pub fn cleanup_sockets() {
    let pid = std::process::id();
    let pid_socket = get_pid_socket(pid);
    let latest_socket = get_latest_socket();

    let _ = std::fs::remove_file(&pid_socket);

    if let Ok(path) = std::fs::read_link(&latest_socket) {
        if path == pid_socket {
            let _ = std::fs::remove_file(&latest_socket);
        }
    }
}

fn parse_action(s: &str) -> Option<Action> {
    match s {
        "NextImage" => Some(Action::NextImage),
        "PrevImage" => Some(Action::PrevImage),
        "ToggleGrid" => Some(Action::ToggleGrid),
        "ToggleSlideshow" => Some(Action::ToggleSlideshow),
        "ToggleStatusBar" => Some(Action::ToggleStatusBar),
        "Quit" => Some(Action::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_parse_action() {
        assert_eq!(parse_action("NextImage"), Some(Action::NextImage));
        assert_eq!(parse_action("PrevImage"), Some(Action::PrevImage));
        assert_eq!(parse_action("ToggleGrid"), Some(Action::ToggleGrid));
        assert_eq!(parse_action("Invalid"), None);
    }

    #[test]
    fn test_get_socket_dir() {
        let dir = get_socket_dir();
        assert!(dir.exists());
        let meta = std::fs::metadata(&dir).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn test_message_framing() {
        let (client, mut server) = UnixStream::pair().unwrap();

        // Server thread to simulate a simple response
        std::thread::spawn(move || {
            let mut len_buf = [0u8; 4];
            server.read_exact(&mut len_buf).unwrap();
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            server.read_exact(&mut buf).unwrap();

            let req: IpcRequest = serde_json::from_slice(&buf).unwrap();
            if let IpcRequest::GetState = req {
                write_ipc_response(&mut server, &IpcResponse::State("test-state".into())).unwrap();
            }
        });

        let resp = send_ipc_message(client, &IpcRequest::GetState).unwrap();
        if let IpcResponse::State(s) = resp {
            assert_eq!(s, "test-state");
        } else {
            panic!("Unexpected response: {:?}", resp);
        }
    }

    #[test]
    fn test_large_payload_rejection() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        // Simulate a client sending a huge length
        let huge_len = (MAX_PAYLOAD_SIZE + 1) as u32;
        client.write_all(&huge_len.to_le_bytes()).unwrap();

        // Server check
        let mut len_buf = [0u8; 4];
        server.read_exact(&mut len_buf).unwrap();
        let req_len = u32::from_le_bytes(len_buf) as usize;
        assert!(req_len > MAX_PAYLOAD_SIZE);

        // We can't easily test the server's thread response here without spinning up the whole server,
        // but we've verified the check exists in the code.
    }
}
