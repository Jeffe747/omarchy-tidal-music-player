use crate::api::{cached_cover_file_url, Playlist, TrackItem};
use crate::cast::CastDevice;
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
    artist_id: u64,
    album_id: u64,
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
            art_url: cached_cover_file_url(track.cover()).unwrap_or_default(),
            artist_id: track
                .artist
                .as_ref()
                .map(|a| a.id)
                .or_else(|| track.artists.first().map(|a| a.id))
                .unwrap_or(0),
            album_id: track.album.as_ref().map(|a| a.id).unwrap_or(0),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerMessage<'a> {
    FavoritesLoaded {
        tracks: Vec<FavoriteTrack<'a>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        offset: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        has_more: Option<bool>,
    },
    FavoritesProgress {
        loaded: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
        tracks: Vec<FavoriteTrack<'a>>,
    },
    FavoritesComplete {
        total: usize,
    },
    FavoriteUpdated {
        track_id: u64,
        is_favorite: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        track: Option<FavoriteTrack<'a>>,
    },
    FavoritesError {
        error: &'a str,
    },
    SearchResults {
        request_id: u64,
        results: Vec<FavoriteTrack<'a>>,
    },
    SearchError {
        request_id: u64,
        error: &'a str,
    },
    PlaylistsLoaded {
        playlists: Vec<Playlist>,
    },
    PlaylistsError {
        error: &'a str,
    },
    PlaylistTracksLoaded {
        playlist_id: &'a str,
        tracks: Vec<FavoriteTrack<'a>>,
    },
    PlaylistTracksError {
        playlist_id: &'a str,
        error: &'a str,
    },
    ExplorationLoaded {
        view: &'a str,
        tracks: Vec<FavoriteTrack<'a>>,
    },
    ExplorationError {
        view: &'a str,
        error: &'a str,
    },
    PlaybackStarted {
        track_id: u64,
    },
    PlaybackError {
        error: &'a str,
    },
    CastDevicesDiscovered {
        devices: Vec<CastDevice>,
    },
    CastError {
        error: &'a str,
    },
    ArtworkLoaded {
        cover: &'a str,
        path: &'a str,
    },
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
    #[serde(default)]
    pub request_id: Option<u64>,
    #[serde(default)]
    pub playlist_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub album_id: Option<u64>,
    #[serde(default)]
    pub artist_id: Option<u64>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub cover: Option<String>,
}

fn default_cast_state() -> String {
    "idle".to_string()
}

fn default_cast_volume() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcStateMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub daemon_version: Option<String>,
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
    #[serde(default)]
    pub preferred_audio_quality: Option<String>,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub repeat_mode: String,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub artist_id: Option<u64>,
    #[serde(default)]
    pub album_id: Option<u64>,
    #[serde(default)]
    pub cast_active: bool,
    #[serde(default)]
    pub cast_device_name: Option<String>,
    #[serde(default = "default_cast_state")]
    pub cast_state: String,
    #[serde(default = "default_cast_volume")]
    pub cast_volume: f64,
    #[serde(default)]
    pub cast_firewall_blocked: bool,
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
            let parsed: IpcCommand =
                serde_json::from_value(serde_json::json!({"command": command})).unwrap();
            assert_eq!(parsed.command, command);
        }
        let seek: IpcCommand =
            serde_json::from_str(r#"{"command":"seek","position":12.5}"#).unwrap();
        assert_eq!(seek.position, Some(12.5));
        for command in [
            "toggle_shuffle",
            "cycle_repeat",
            "toggle_favorite",
            "get_album_tracks",
            "get_artist_tracks",
            "get_track_radio",
        ] {
            let parsed: IpcCommand =
                serde_json::from_value(serde_json::json!({"command": command})).unwrap();
            assert_eq!(parsed.command, command);
        }
        let album: IpcCommand =
            serde_json::from_value(serde_json::json!({"command":"get_album_tracks","album_id":12}))
                .unwrap();
        assert_eq!(album.album_id, Some(12));
        let radio: IpcCommand = serde_json::from_value(
            serde_json::json!({"command":"get_track_radio","track_id":42}),
        )
        .unwrap();
        assert_eq!(radio.track_id, Some(42));
    }

    #[test]
    fn test_song_context_commands_deserialize_optional_track_ids() {
        let add: IpcCommand = serde_json::from_str(r#"{"command":"add_playlist_track","playlist_id":"abc","track_id":42}"#).unwrap();
        assert_eq!(add.command, "add_playlist_track");
        assert_eq!(add.playlist_id.as_deref(), Some("abc"));
        assert_eq!(add.track_id, Some(42));
        let favorite: IpcCommand = serde_json::from_str(r#"{"command":"toggle_favorite","track_id":42}"#).unwrap();
        assert_eq!(favorite.track_id, Some(42));
        let current: IpcCommand = serde_json::from_str(r#"{"command":"toggle_favorite"}"#).unwrap();
        assert_eq!(current.track_id, None);
    }

    #[test]
    fn test_ipc_state_message_serialization() {
        let msg = IpcStateMessage {
            msg_type: "status".to_string(),
            daemon_version: Some("1.0.59".to_string()),
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
            preferred_audio_quality: Some("LOSSLESS".to_string()),
            shuffle: false,
            repeat_mode: "off".into(),
            is_favorite: true,
            artist_id: Some(11),
            album_id: Some(22),
            cast_active: false,
            cast_device_name: None,
            cast_state: "idle".to_string(),
            cast_volume: 1.0,
            cast_firewall_blocked: true,
        };

        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains(r#""type":"status""#));
        assert!(serialized.contains(r#""authenticated":true"#));
        assert!(serialized.contains(r#""cast_active":false"#));
        assert!(serialized.contains(r#""cast_state":"idle""#));
        assert!(serialized.contains(r#""cast_volume":1.0"#));
        assert!(serialized.contains(r#""cast_firewall_blocked":true"#));

        let deserialized: IpcStateMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, msg);
    }

    #[test]
    fn test_cast_commands_and_messages_serialization() {
        let discover: IpcCommand =
            serde_json::from_str(r#"{"command":"discover_cast_devices"}"#).unwrap();
        assert_eq!(discover.command, "discover_cast_devices");

        let refresh: IpcCommand =
            serde_json::from_str(r#"{"command":"refresh_cast_devices"}"#).unwrap();
        assert_eq!(refresh.command, "refresh_cast_devices");

        let connect: IpcCommand =
            serde_json::from_str(r#"{"command":"connect_cast","device_id":"cast-123"}"#).unwrap();
        assert_eq!(connect.command, "connect_cast");
        assert_eq!(connect.device_id.as_deref(), Some("cast-123"));

        let disconnect: IpcCommand =
            serde_json::from_str(r#"{"command":"disconnect_cast"}"#).unwrap();
        assert_eq!(disconnect.command, "disconnect_cast");

        let vol: IpcCommand =
            serde_json::from_str(r#"{"command":"set_cast_volume","volume":0.75}"#).unwrap();
        assert_eq!(vol.command, "set_cast_volume");
        assert_eq!(vol.volume, Some(0.75));

        let device = CastDevice {
            id: "device-1".to_string(),
            name: "Living Room Speaker".to_string(),
            model: "Chromecast Audio".to_string(),
            ip: "192.168.1.50".to_string(),
            port: 8009,
        };
        let msg = PlayerMessage::CastDevicesDiscovered {
            devices: vec![device.clone()],
        };
        let val = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "cast_devices_discovered");
        assert_eq!(val["devices"][0]["name"], "Living Room Speaker");
        assert_eq!(val["devices"][0]["model"], "Chromecast Audio");

        let err_msg = PlayerMessage::CastError {
            error: "Device connection timed out",
        };
        let err_val = serde_json::to_value(&err_msg).unwrap();
        assert_eq!(err_val["type"], "cast_error");
        assert_eq!(err_val["error"], "Device connection timed out");
    }

    #[test]
    fn test_player_message_shapes() {
        let track: TrackItem = serde_json::from_str(r#"{"id":42,"title":"Song","duration":180,"artists":[{"name":"Artist"}],"album":{"title":"Album","cover":"ab-cd"}}"#).unwrap();
        let msg = PlayerMessage::FavoritesLoaded {
            tracks: vec![FavoriteTrack::from(&track)],
            offset: None,
            total: None,
            has_more: None,
        };
        let value = serde_json::to_value(msg).unwrap();
        assert_eq!(value["type"], "favorites_loaded");
        assert_eq!(
            value["tracks"][0],
            serde_json::json!({
                "id":42,"title":"Song","artist":"Artist","album":"Album","duration":180,
                "cover":"ab-cd","art_url":"","artist_id":0,"album_id":0
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
        let search = PlayerMessage::SearchResults {
            request_id: 7,
            results: vec![FavoriteTrack::from(&track)],
        };
        let value = serde_json::to_value(search).unwrap();
        assert_eq!(value["type"], "search_results");
        assert_eq!(value["results"][0]["id"], 42);

        let paged_favs = PlayerMessage::FavoritesLoaded {
            tracks: vec![FavoriteTrack::from(&track)],
            offset: Some(0),
            total: Some(1900),
            has_more: Some(true),
        };
        let paged_val = serde_json::to_value(paged_favs).unwrap();
        assert_eq!(paged_val["type"], "favorites_loaded");
        assert_eq!(paged_val["offset"], 0);
        assert_eq!(paged_val["total"], 1900);
        assert_eq!(paged_val["has_more"], true);

        let progress = PlayerMessage::FavoritesProgress {
            loaded: 200,
            total: Some(1900),
            tracks: vec![FavoriteTrack::from(&track)],
        };
        let prog_val = serde_json::to_value(progress).unwrap();
        assert_eq!(prog_val["type"], "favorites_progress");
        assert_eq!(prog_val["loaded"], 200);
        assert_eq!(prog_val["total"], 1900);

        let complete = PlayerMessage::FavoritesComplete { total: 1900 };
        let comp_val = serde_json::to_value(complete).unwrap();
        assert_eq!(comp_val["type"], "favorites_complete");
        assert_eq!(comp_val["total"], 1900);

        let fav_upd = PlayerMessage::FavoriteUpdated {
            track_id: 42,
            is_favorite: true,
            track: Some(FavoriteTrack::from(&track)),
        };
        let fav_upd_val = serde_json::to_value(fav_upd).unwrap();
        assert_eq!(fav_upd_val["type"], "favorite_updated");
        assert_eq!(fav_upd_val["track_id"], 42);
        assert_eq!(fav_upd_val["is_favorite"], true);
        assert_eq!(fav_upd_val["track"]["id"], 42);
        assert_eq!(fav_upd_val["track"]["title"], "Song");
    }
}
