use base64::prelude::*;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};

#[derive(Debug, Clone, Deserialize)]
pub struct BtsManifest {
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub codecs: Option<String>,
    #[serde(rename = "encryptionType")]
    pub encryption_type: Option<String>,
    pub urls: Vec<String>,
}

pub struct PlaybackEngine {
    mpv_socket_path: PathBuf,
    mpv_process: Option<Child>,
}

impl PlaybackEngine {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let mpv_socket_path = PathBuf::from(runtime_dir).join("tidal-mpv.sock");
        Self {
            mpv_socket_path,
            mpv_process: None,
        }
    }

    pub fn parse_stream_url(manifest_mime: &str, base64_data: &str) -> Result<String, String> {
        let decoded = BASE64_STANDARD
            .decode(base64_data.trim())
            .map_err(|e| format!("Base64 decode error: {e}"))?;

        if manifest_mime == "application/vnd.tidal.bts" {
            let manifest: BtsManifest = serde_json::from_slice(&decoded)
                .map_err(|e| format!("Failed to parse BTS manifest JSON: {e}"))?;

            if let Some(url) = manifest.urls.first() {
                Ok(url.clone())
            } else {
                Err("No stream URLs found in BTS manifest".to_string())
            }
        } else if manifest_mime == "application/dash+xml" {
            // For DASH, write temporary MPD or pipe to mpv
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
            let mpd_path = PathBuf::from(runtime_dir).join("tidal-stream.mpd");
            std::fs::write(&mpd_path, decoded).map_err(|e| format!("Failed to write MPD: {e}"))?;
            Ok(mpd_path.to_string_lossy().to_string())
        } else {
            Err(format!("Unsupported manifest MIME type: {manifest_mime}"))
        }
    }

    pub fn ensure_mpv_running(&mut self) -> Result<(), String> {
        if self.mpv_socket_path.exists() {
            return Ok(());
        }

        let child = Command::new("mpv")
            .arg("--idle=yes")
            .arg(format!("--input-ipc-server={}", self.mpv_socket_path.to_string_lossy()))
            .arg("--no-video")
            .arg("--ao=pipewire")
            .spawn()
            .map_err(|e| format!("Failed to start mpv: {e}"))?;

        self.mpv_process = Some(child);
        // Short pause to allow mpv to bind socket
        std::thread::sleep(std::time::Duration::from_millis(150));
        Ok(())
    }

    pub fn send_mpv_command(&self, cmd: serde_json::Value) -> Result<serde_json::Value, String> {
        let mut stream = UnixStream::connect(&self.mpv_socket_path)
            .map_err(|e| format!("Failed to connect to mpv socket: {e}"))?;

        let line = format!("{}\n", cmd);
        stream.write_all(line.as_bytes()).map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).map_err(|e| e.to_string())?;

        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    pub fn load_url(&mut self, url: &str) -> Result<(), String> {
        self.ensure_mpv_running()?;
        let cmd = serde_json::json!({
            "command": ["loadfile", url, "replace"]
        });
        self.send_mpv_command(cmd)?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parsed.unwrap(), "https://audio.tidal.com/stream/track123.flac");
    }

    #[test]
    fn test_parse_invalid_manifest() {
        let parsed = PlaybackEngine::parse_stream_url("application/unknown", "invalid");
        assert!(parsed.is_err());
    }
}
