use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcCommand {
    pub command: String,
    #[serde(default)]
    pub track_id: Option<u64>,
    #[serde(default)]
    pub position: Option<f64>,
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcStateMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub authenticated: bool,
    pub is_playing: bool,
    #[serde(default)]
    pub track_title: Option<String>,
    #[serde(default)]
    pub track_artist: Option<String>,
    #[serde(default)]
    pub track_album: Option<String>,
    #[serde(default)]
    pub track_art_url: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub position: Option<f64>,
    #[serde(default)]
    pub audio_quality: Option<String>,
}

#[derive(Clone)]
pub struct IpcServer {
    socket_path: PathBuf,
    clients: Arc<Mutex<Vec<UnixStream>>>,
}

impl IpcServer {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let socket_path = PathBuf::from(runtime_dir).join("tidal.sock");
        Self {
            socket_path,
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[allow(dead_code)]
    pub fn with_path(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub fn bind(&self) -> std::io::Result<UnixListener> {
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }
        UnixListener::bind(&self.socket_path)
    }

    pub fn register_client(&self, stream: UnixStream) {
        if let Ok(mut clients) = self.clients.lock() {
            clients.push(stream);
        }
    }

    pub fn broadcast(&self, msg: &serde_json::Value) {
        let payload = format!("{}\n", msg);
        let bytes = payload.as_bytes();
        if let Ok(mut clients) = self.clients.lock() {
            clients.retain_mut(|client| {
                client.write_all(bytes).and_then(|_| client.flush()).is_ok()
            });
        }
    }

    #[allow(dead_code)]
    pub fn send_message(stream: &mut UnixStream, msg: &serde_json::Value) -> std::io::Result<()> {
        let payload = format!("{}\n", msg);
        stream.write_all(payload.as_bytes())?;
        stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_command_deserialization() {
        let json = r#"{"command":"start_auth"}"#;
        let cmd: Result<IpcCommand, _> = serde_json::from_str(json);
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        assert_eq!(cmd.command, "start_auth");
        assert_eq!(cmd.track_id, None);

        let json_with_params = r#"{"command":"play_track","track_id":12345}"#;
        let cmd: IpcCommand = serde_json::from_str(json_with_params).unwrap();
        assert_eq!(cmd.command, "play_track");
        assert_eq!(cmd.track_id, Some(12345));
    }

    #[test]
    fn test_ipc_state_message_serialization() {
        let msg = IpcStateMessage {
            msg_type: "status".to_string(),
            authenticated: true,
            is_playing: false,
            track_title: Some("Song Title".to_string()),
            track_artist: Some("Artist Name".to_string()),
            track_album: Some("Album Name".to_string()),
            track_art_url: None,
            duration: Some(180.0),
            position: Some(12.5),
            audio_quality: Some("LOSSLESS".to_string()),
        };

        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains(r#""type":"status""#));
        assert!(serialized.contains(r#""authenticated":true"#));

        let deserialized: IpcStateMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, msg);
    }
}
