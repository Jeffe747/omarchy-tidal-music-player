use crate::api::{cover_url, TrackItem};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
pub struct FavoriteTrack<'a> {
    id: u64,
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    duration: u64,
    cover: &'a str,
    art_url: String,
}

impl<'a> From<&'a TrackItem> for FavoriteTrack<'a> {
    fn from(track: &'a TrackItem) -> Self {
        Self {
            id: track.id,
            title: &track.title,
            artist: track.artist_name(),
            album: track.album_title(),
            duration: track.duration,
            cover: track.cover(),
            art_url: cover_url(track.cover()),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerMessage<'a> {
    FavoritesLoaded { tracks: Vec<FavoriteTrack<'a>> },
    FavoritesError { error: &'a str },
    SearchResults { results: Vec<FavoriteTrack<'a>> },
    SearchError { error: &'a str },
    PlaybackStarted { track_id: u64 },
    PlaybackError { error: &'a str },
}

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
    pub track_id: Option<u64>,
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
        if let Err(error) = stream.set_write_timeout(Some(std::time::Duration::from_secs(3))) {
            eprintln!("Failed to configure IPC client: {error}");
            return;
        }
        if let Ok(mut clients) = self.clients.lock() {
            clients.push(stream);
        }
    }

    pub fn broadcast(&self, msg: &impl Serialize) {
        let mut payload = match serde_json::to_string(msg) {
            Ok(payload) => payload,
            Err(error) => {
                eprintln!("Failed to serialize IPC message: {error}");
                return;
            }
        };
        payload.push('\n');
        let bytes = payload.as_bytes();
        if let Ok(mut clients) = self.clients.lock() {
            clients
                .retain_mut(|client| client.write_all(bytes).and_then(|_| client.flush()).is_ok());
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
    fn test_transport_command_shapes() {
        for command in ["pause", "resume", "play", "toggle_play", "next", "previous"] {
            let parsed: IpcCommand = serde_json::from_value(serde_json::json!({"command": command})).unwrap();
            assert_eq!(parsed.command, command);
        }
        let seek: IpcCommand = serde_json::from_str(r#"{"command":"seek","position":12.5}"#).unwrap();
        assert_eq!(seek.position, Some(12.5));
    }

    #[test]
    fn test_ipc_state_message_serialization() {
        let msg = IpcStateMessage {
            msg_type: "status".to_string(),
            authenticated: true,
            is_playing: false,
            track_id: Some(42),
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

    #[test]
    fn test_player_message_shapes() {
        let track: TrackItem = serde_json::from_str(r#"{"id":42,"title":"Song","duration":180,"artists":[{"name":"Artist"}],"album":{"title":"Album","cover":"ab-cd"}}"#).unwrap();
        let msg = PlayerMessage::FavoritesLoaded {
            tracks: vec![FavoriteTrack::from(&track)],
        };
        let value = serde_json::to_value(msg).unwrap();
        assert_eq!(value["type"], "favorites_loaded");
        assert_eq!(
            value["tracks"][0],
            serde_json::json!({
                "id":42,"title":"Song","artist":"Artist","album":"Album","duration":180,
                "cover":"ab-cd","art_url":"https://resources.tidal.com/images/ab/cd/640x640.jpg"
            })
        );
        assert_eq!(
            serde_json::to_value(PlayerMessage::PlaybackStarted { track_id: 42 }).unwrap(),
            serde_json::json!({"type":"playback_started","track_id":42})
        );
        for (message, kind) in [
            (
                PlayerMessage::FavoritesError {
                    error: "No session",
                },
                "favorites_error",
            ),
            (
                PlayerMessage::PlaybackError {
                    error: "No session",
                },
                "playback_error",
            ),
        ] {
            assert_eq!(
                serde_json::to_value(message).unwrap(),
                serde_json::json!({"type":kind,"error":"No session"})
            );
        }
        let search = PlayerMessage::SearchResults { results: vec![FavoriteTrack::from(&track)] };
        let value = serde_json::to_value(search).unwrap();
        assert_eq!(value["type"], "search_results");
        assert_eq!(value["results"][0]["id"], 42);
    }
}
