use crate::app::AppEvent;
use crate::keybinds::Action;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
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

#[derive(Debug)]
pub enum IpcError {
    Io(std::io::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    InvalidMessageType,
    InvalidAction(String),
    InvalidPath(String),
    ResponseTooLarge,
    NoTargets,
    ServerError(String),
    ConnectFailed(PathBuf),
    NoSuccess,
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::Serialize(err) => write!(f, "Failed to serialize request: {}", err),
            Self::Deserialize(err) => write!(f, "Failed to parse response: {}", err),
            Self::InvalidMessageType => {
                write!(f, "Invalid message type. Use 'add', 'cmd', or 'state'.")
            }
            Self::InvalidAction(action) => write!(f, "Invalid action: {}", action),
            Self::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            Self::ResponseTooLarge => write!(f, "Response payload too large"),
            Self::NoTargets => write!(f, "No running instances of rsiv found."),
            Self::ServerError(msg) => write!(f, "Server error: {}", msg),
            Self::ConnectFailed(path) => write!(f, "Could not connect to instance at {:?}", path),
            Self::NoSuccess => {
                write!(f, "Failed to send message. Target instances may have crashed.")
            }
        }
    }
}

impl Error for IpcError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Serialize(err) => Some(err),
            Self::Deserialize(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for IpcError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
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

fn send_ipc_message(mut stream: UnixStream, req: &IpcRequest) -> Result<IpcResponse, IpcError> {
    let payload = serde_json::to_vec(req).map_err(IpcError::Serialize)?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&payload)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u32::from_le_bytes(len_buf) as usize;

    if resp_len > MAX_PAYLOAD_SIZE {
        return Err(IpcError::ResponseTooLarge);
    }

    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf)?;

    serde_json::from_slice(&resp_buf).map_err(IpcError::Deserialize)
}

// Client
pub fn send_message(
    msg_type: &str,
    payload: &str,
    target_pid: Option<u32>,
    broadcast_all: bool,
) -> Result<(), IpcError> {
    let req = match msg_type {
        "add" => {
            let path = Path::new(payload);
            let abs_path =
                std::fs::canonicalize(path).map_err(|e| IpcError::InvalidPath(e.to_string()))?;
            IpcRequest::AddFile(abs_path)
        }
        "cmd" => {
            if let Some(action) = parse_action(payload) {
                IpcRequest::RunAction(action)
            } else {
                return Err(IpcError::InvalidAction(payload.to_string()));
            }
        }
        "state" => IpcRequest::GetState,
        _ => return Err(IpcError::InvalidMessageType),
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
        return Err(IpcError::NoTargets);
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
                    return Err(IpcError::ServerError(e));
                }
                Err(e) => return Err(e),
            }
        } else if !broadcast_all {
            let _ = std::fs::remove_file(&target); // Clean up dead socket
            return Err(IpcError::ConnectFailed(target));
        }
    }

    if success_count == 0 {
        return Err(IpcError::NoSuccess);
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
