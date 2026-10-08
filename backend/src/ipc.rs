use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcCommand {
    pub command: String,
    pub track_id: Option<u64>,
    pub position: Option<f64>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcStateMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub authenticated: bool,
    pub is_playing: bool,
    pub track_title: Option<String>,
    pub track_artist: Option<String>,
    pub track_album: Option<String>,
    pub track_art_url: Option<String>,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    pub audio_quality: Option<String>,
}

pub struct IpcServer {
    socket_path: PathBuf,
}

impl IpcServer {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let socket_path = PathBuf::from(runtime_dir).join("tidal.sock");
        Self { socket_path }
    }

    pub fn bind(&self) -> std::io::Result<UnixListener> {
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }
        UnixListener::bind(&self.socket_path)
    }

    pub fn send_message(stream: &mut UnixStream, msg: &serde_json::Value) -> std::io::Result<()> {
        let payload = format!("{}\n", msg);
        stream.write_all(payload.as_bytes())?;
        stream.flush()
    }
}
