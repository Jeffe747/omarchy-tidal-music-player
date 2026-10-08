use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2/device/authorization";
const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";
// Common client ID used for Tidal device flow in community players
pub const DEFAULT_CLIENT_ID: &str = "zU4XHVVk3BmICqXd";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthInfo {
    #[serde(rename = "deviceCode")]
    pub device_code: String,
    #[serde(rename = "userCode")]
    pub user_code: String,
    #[serde(rename = "verificationUriComplete")]
    pub verification_uri: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub user_id: Option<u64>,
    pub expires_in: Option<u64>,
}

pub struct AuthManager {
    client_id: String,
}

impl AuthManager {
    pub fn new(client_id: Option<String>) -> Self {
        Self {
            client_id: client_id.unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()),
        }
    }

    pub fn session_file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".local/state/omarchy/tidal/session.json")
    }

    pub fn load_session(&self) -> Option<Session> {
        let path = Self::session_file_path();
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save_session(&self, session: &Session) -> std::io::Result<()> {
        let path = Self::session_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(session)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn request_device_code(&self) -> Result<DeviceAuthInfo, String> {
        let resp = ureq::post(AUTH_URL)
            .send_form(&[
                ("client_id", self.client_id.as_str()),
                ("scope", "r_usr w_usr"),
            ])
            .map_err(|e| format!("Auth request failed: {e}"))?;

        resp.into_json::<DeviceAuthInfo>()
            .map_err(|e| format!("Failed to parse device auth response: {e}"))
    }

    pub fn poll_token(&self, device_code: &str) -> Result<Session, String> {
        let resp = ureq::post(TOKEN_URL)
            .send_form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("scope", "r_usr w_usr"),
            ])
            .map_err(|e| format!("Token poll failed: {e}"))?;

        resp.into_json::<Session>()
            .map_err(|e| format!("Failed to parse session token: {e}"))
    }
}
