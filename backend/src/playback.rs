use base64::prelude::*;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Deserialize)]
pub struct BtsManifest {
    #[serde(rename = "encryptionType")]
    pub encryption_type: Option<String>,
    pub urls: Vec<String>,
}

pub struct PlaybackEngine {
    mpv_socket_path: PathBuf,
    mpv_process: Option<Child>,
    mpd_path: Option<PathBuf>,
}

impl PlaybackEngine {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let mpv_socket_path = PathBuf::from(runtime_dir).join("tidal-mpv.sock");
        Self {
            mpv_socket_path,
            mpv_process: None,
            mpd_path: None,
        }
    }

    pub fn parse_stream_url(manifest_mime: &str, base64_data: &str) -> Result<String, String> {
        let decoded = BASE64_STANDARD
            .decode(base64_data.trim())
            .map_err(|e| format!("Base64 decode error: {e}"))?;

        if manifest_mime == "application/vnd.tidal.bts" {
            let manifest: BtsManifest = serde_json::from_slice(&decoded)
                .map_err(|e| format!("Failed to parse BTS manifest JSON: {e}"))?;

            if manifest.encryption_type.as_deref() != Some("NONE") {
                return Err(
                    "Encrypted BTS streams are unsupported (encryptionType must be NONE)"
                        .to_string(),
                );
            }
            if let Some(url) = manifest.urls.first() {
                Ok(url.clone())
            } else {
                Err("No stream URLs found in BTS manifest".to_string())
            }
        } else if manifest_mime == "application/dash+xml" {
            // For DASH, write temporary MPD or pipe to mpv
            let runtime_dir =
                std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
            let mpd_path = PathBuf::from(runtime_dir).join("tidal-stream.mpd");
            std::fs::write(&mpd_path, decoded).map_err(|e| format!("Failed to write MPD: {e}"))?;
            Ok(mpd_path.to_string_lossy().to_string())
        } else {
            Err(format!("Unsupported manifest MIME type: {manifest_mime}"))
        }
    }

    pub fn ensure_mpv_running(&mut self) -> Result<(), String> {
        self.start_mpv(std::ffi::OsStr::new("mpv"))
    }

    pub fn resolve_stream_url(&mut self, mime: &str, data: &str) -> Result<String, String> {
        let url = Self::parse_stream_url(mime, data)?;
        if mime == "application/dash+xml" {
            self.mpd_path = Some(PathBuf::from(&url));
        }
        Ok(url)
    }

    fn start_mpv(&mut self, program: &std::ffi::OsStr) -> Result<(), String> {
        if let Some(child) = self.mpv_process.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                self.stop_process()?;
            }
        }
        if UnixStream::connect(&self.mpv_socket_path).is_ok() {
            return Ok(());
        }
        self.stop_process()?;
        if self.mpv_socket_path.exists() {
            std::fs::remove_file(&self.mpv_socket_path)
                .map_err(|e| format!("Failed to remove stale mpv socket: {e}"))?;
        }
        let child = Command::new(program)
            .arg("--idle=yes")
            .arg(format!(
                "--input-ipc-server={}",
                self.mpv_socket_path.to_string_lossy()
            ))
            .arg("--no-video")
            .arg("--no-terminal")
            .arg("--ao=pipewire")
            .spawn()
            .map_err(|e| format!("Failed to start mpv: {e}"))?;

        self.mpv_process = Some(child);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            if UnixStream::connect(&self.mpv_socket_path).is_ok() {
                return Ok(());
            }
            if let Some(child) = self.mpv_process.as_mut() {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    self.stop_process()?;
                    return Err(format!("mpv exited before its socket was ready: {status}"));
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        self.stop_process()?;
        Err("Timed out waiting for mpv IPC socket".to_string())
    }

    pub fn send_mpv_command(&self, cmd: serde_json::Value) -> Result<serde_json::Value, String> {
        let mut stream = UnixStream::connect(&self.mpv_socket_path)
            .map_err(|e| format!("Failed to connect to mpv socket: {e}"))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| e.to_string())?;
        let mut cmd = cmd;
        cmd["request_id"] = serde_json::json!(1);
        let line = format!("{}\n", cmd);
        stream
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut response = String::new();
            if reader.read_line(&mut response).map_err(|e| e.to_string())? == 0 {
                return Err("mpv closed its IPC connection".to_string());
            }
            let response: serde_json::Value =
                serde_json::from_str(&response).map_err(|e| e.to_string())?;
            if response["request_id"] == 1 {
                if response["error"] != "success" {
                    return Err(format!("mpv command failed: {}", response["error"]));
                }
                return Ok(response);
            }
            if Instant::now() >= deadline {
                return Err("Timed out waiting for mpv command response".to_string());
            }
        }
    }

    pub fn load_url(&mut self, url: &str) -> Result<(), String> {
        self.ensure_mpv_running()?;
        let cmd = serde_json::json!({
            "command": ["loadfile", url, "replace"]
        });
        self.send_mpv_command(cmd)?;
        self.set_pause(false)?;
        Ok(())
    }

    pub fn set_pause(&self, paused: bool) -> Result<(), String> {
        self.send_mpv_command(serde_json::json!({"command": ["set_property", "pause", paused]}))?;
        Ok(())
    }

    pub fn property(&self, name: &str) -> Result<serde_json::Value, String> {
        let response =
            self.send_mpv_command(serde_json::json!({"command": ["get_property", name]}))?;
        Ok(response["data"].clone())
    }

    fn stop_process(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.mpv_process.take() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                child
                    .kill()
                    .map_err(|e| format!("Failed to stop mpv: {e}"))?;
            }
            child
                .wait()
                .map_err(|e| format!("Failed to reap mpv: {e}"))?;
            if self.mpv_socket_path.exists() {
                std::fs::remove_file(&self.mpv_socket_path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.stop_process()?;
        if let Some(path) = self.mpd_path.take() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        let cmd = serde_json::json!({
            "command": ["cycle", "pause"]
        });
        self.send_mpv_command(cmd)?;
        Ok(())
    }

    pub fn seek(&self, seconds: f64) -> Result<(), String> {
        let cmd = serde_json::json!({
            "command": ["seek", seconds, "absolute"]
        });
        self.send_mpv_command(cmd)?;
        Ok(())
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Playback cleanup failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "tidal-mpv-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn engine(&self) -> PlaybackEngine {
            PlaybackEngine {
                mpv_socket_path: self.0.join("tidal-mpv.sock"),
                mpv_process: None,
                mpd_path: None,
            }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn test_mpv_stale_socket_commands_and_cleanup() {
        let dir = TestDir::new();
        let mut engine = dir.engine();
        let stale = std::os::unix::net::UnixListener::bind(&engine.mpv_socket_path).unwrap();
        drop(stale);
        let program =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/playback/fake-mpv.py");
        engine.start_mpv(program.as_os_str()).unwrap();
        let pid = engine.mpv_process.as_ref().unwrap().id();
        let args: Vec<String> = serde_json::from_str(
            &std::fs::read_to_string(engine.mpv_socket_path.with_extension("sock.args")).unwrap(),
        )
        .unwrap();
        for arg in ["--idle=yes", "--no-video", "--no-terminal", "--ao=pipewire"] {
            assert!(args.iter().any(|value| value == arg));
        }
        engine.load_url("https://example.com/test.flac").unwrap();
        assert_eq!(engine.property("pause").unwrap(), false);
        assert_eq!(engine.property("idle-active").unwrap(), false);
        let player = crate::Player {
            engine,
            favorites: Vec::new(),
            current: Some(serde_json::from_str(r#"{"id":42,"title":"Song","duration":180,"artist":{"name":"Artist"},"album":{"title":"Album","cover":"ab-cd"}}"#).unwrap()),
            quality: "LOSSLESS".to_string(),
        };
        let auth = crate::auth::AuthManager::new(None);
        let status = crate::build_status_message(&auth, &player);
        assert!(status.is_playing);
        assert_eq!(status.track_id, Some(42));
        assert_eq!(status.track_title.as_deref(), Some("Song"));
        assert_eq!(status.track_artist.as_deref(), Some("Artist"));
        assert_eq!(status.track_album.as_deref(), Some("Album"));
        assert_eq!(
            status.track_art_url.as_deref(),
            Some("https://resources.tidal.com/images/ab/cd/640x640.jpg")
        );
        assert_eq!(status.duration, Some(180.0));
        player.engine.toggle_pause().unwrap();
        assert!(!crate::build_status_message(&auth, &player).is_playing);
        engine = player.engine;
        assert_eq!(engine.property("pause").unwrap(), true);
        assert!(engine
            .load_url("fail")
            .unwrap_err()
            .contains("loading failed"));
        let commands =
            std::fs::read_to_string(engine.mpv_socket_path.with_extension("sock.commands"))
                .unwrap();
        assert!(commands.contains(r#"["loadfile", "https://example.com/test.flac", "replace"]"#));
        assert!(commands.contains(r#"["set_property", "pause", false]"#));
        drop(engine);
        assert!(!dir.0.join("tidal-mpv.sock").exists());
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "mpv child must be reaped"
        );
    }

    #[test]
    fn test_mpv_start_failure_is_reported_and_reaped() {
        let dir = TestDir::new();
        let mut engine = dir.engine();
        assert!(engine
            .start_mpv(std::ffi::OsStr::new("/bin/false"))
            .is_err());
        assert!(engine.mpv_process.is_none());
    }

    #[test]
    fn test_real_mpv_stale_socket_and_drop() {
        let dir = TestDir::new();
        let mut engine = dir.engine();
        let stale = std::os::unix::net::UnixListener::bind(&engine.mpv_socket_path).unwrap();
        drop(stale);
        engine.ensure_mpv_running().unwrap();
        let pid = engine.mpv_process.as_ref().unwrap().id();
        let args = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap();
        let args = String::from_utf8(args).unwrap();
        for arg in ["--idle=yes", "--no-video", "--no-terminal", "--ao=pipewire"] {
            assert!(args.split('\0').any(|value| value == arg));
        }
        assert_eq!(engine.property("idle-active").unwrap(), true);
        engine.ensure_mpv_running().unwrap();
        assert_eq!(engine.mpv_process.as_ref().unwrap().id(), pid);
        engine.mpv_process.as_mut().unwrap().kill().unwrap();
        engine.mpv_process.as_mut().unwrap().wait().unwrap();
        engine.ensure_mpv_running().unwrap();
        let replacement = engine.mpv_process.as_ref().unwrap().id();
        assert_ne!(pid, replacement);
        drop(engine);
        assert!(!PathBuf::from(format!("/proc/{replacement}")).exists());
        assert!(!dir.0.join("tidal-mpv.sock").exists());
    }

    #[test]
    fn test_parse_bts_manifest() {
        let manifest_json = r#"{
            "mimeType": "audio/flac",
            "codecs": "flac",
            "encryptionType": "NONE",
            "urls": ["https://audio.tidal.com/stream/track123.flac"]
        }"#;
        let b64 = BASE64_STANDARD.encode(manifest_json);
        let parsed = PlaybackEngine::parse_stream_url("application/vnd.tidal.bts", &b64);
        assert!(parsed.is_ok());
        assert_eq!(
            parsed.unwrap(),
            "https://audio.tidal.com/stream/track123.flac"
        );
    }

    #[test]
    fn test_parse_invalid_manifest() {
        let parsed = PlaybackEngine::parse_stream_url("application/unknown", "invalid");
        assert!(parsed.is_err());
    }

    #[test]
    fn test_reject_encrypted_and_empty_bts() {
        for json in [
            r#"{"encryptionType":"OLD_AES","urls":["https://example.com"]}"#,
            r#"{"urls":["https://example.com"]}"#,
            r#"{"encryptionType":"NONE","urls":[]}"#,
        ] {
            assert!(PlaybackEngine::parse_stream_url(
                "application/vnd.tidal.bts",
                &BASE64_STANDARD.encode(json)
            )
            .is_err());
        }
    }
}
