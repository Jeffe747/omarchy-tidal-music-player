use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2/device_authorization";
const FALLBACK_AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2/device/authorization";
const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";

pub const DEFAULT_CLIENT_ID: &str = "4N3n6Q1x95LL5K7p";
const DEFAULT_AUTH_TOKEN_BYTES: &[u8] = &[
    111, 75, 79, 88, 102, 74, 87, 51, 55, 49, 99, 88, 54, 120, 97, 90, 48, 80, 121, 104, 103,
    71, 78, 66, 100, 78, 76, 108, 66, 90, 100, 52, 65, 75, 75, 89, 111, 117, 103, 77, 106,
    105, 107, 61,
];

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub user_id: Option<u64>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(
        default,
        alias = "countryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub country_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PollResult {
    Success(Session),
    Pending,
    SlowDown,
    Expired,
    Denied,
    Error(String),
}

#[derive(Debug, Deserialize)]
struct RawTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    user_id: Option<u64>,
    user: Option<RawUser>,
    #[serde(default, rename = "countryCode")]
    country_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    #[serde(rename = "userId")]
    user_id: Option<u64>,
    #[serde(default, rename = "countryCode")]
    country_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

pub struct AuthManager {
    client_id: String,
    client_secret: Option<String>,
    session_country: Mutex<Option<(String, String)>>,
}

pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl AuthManager {
    pub fn new(client_id: Option<String>) -> Self {
        let env_id = std::env::var("TIDAL_CLIENT_ID").ok();
        let env_secret = std::env::var("TIDAL_CLIENT_SECRET").ok();

        let id = client_id
            .or(env_id)
            .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string());
        let secret = env_secret.or_else(|| {
            if id == DEFAULT_CLIENT_ID {
                Some(std::str::from_utf8(DEFAULT_AUTH_TOKEN_BYTES).unwrap().to_string())
            } else {
                None
            }
        });
        Self {
            client_id: id,
            client_secret: secret,
            session_country: Mutex::new(None),
        }
    }

    fn check_credentials(&self) -> Result<(), String> {
        if self.client_id == DEFAULT_CLIENT_ID
            && self
                .client_secret
                .as_deref()
                .is_none_or(|secret| secret.trim().is_empty())
        {
            return Err(
                "No client secret is configured for this Tidal client"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn with_credentials(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            session_country: Mutex::new(None),
        }
    }

    pub fn session_file_path() -> PathBuf {
        if let Ok(path) = std::env::var("TIDAL_SESSION_PATH") {
            return PathBuf::from(path);
        }
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
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
            }
        }
        let json = serde_json::to_string_pretty(session)?;
        fs::write(&path, json)?;
        #[cfg(unix)]
        {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn delete_session(&self) -> std::io::Result<()> {
        let path = Self::session_file_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        *self
            .session_country
            .lock()
            .map_err(|_| std::io::Error::other("Failed to lock session country cache"))? = None;
        Ok(())
    }

    #[cfg(test)]
    pub fn is_authenticated(&self) -> bool {
        self.load_session()
            .map(|s| !s.access_token.is_empty())
            .unwrap_or(false)
    }

    pub fn is_token_expired(&self, session: &Session) -> bool {
        if let Some(expires_at) = session.expires_at {
            let now = current_unix_timestamp();
            // 60-second safety window before expiration
            now + 60 >= expires_at
        } else {
            false
        }
    }

    pub fn request_device_code(&self) -> Result<DeviceAuthInfo, String> {
        self.check_credentials()?;
        let mut form_data = vec![
            ("client_id", self.client_id.as_str()),
            ("scope", "r_usr w_usr"),
        ];
        if let Some(ref secret) = self.client_secret {
            form_data.push(("client_secret", secret.as_str()));
        }

        // Try primary device_authorization endpoint, fallback if necessary
        crate::log::write("API POST /v1/oauth2/device_authorization");
        let resp = match ureq::post(AUTH_URL)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_form(&form_data)
        {
            Ok(r) => Ok(r),
            Err(ureq::Error::Status(404, _)) => {
                crate::log::write("API POST /v1/oauth2/device_authorization failed: HTTP 404");
                crate::log::write("API POST /v1/oauth2/device/authorization");
                ureq::post(FALLBACK_AUTH_URL)
                    .set("Content-Type", "application/x-www-form-urlencoded")
                    .send_form(&form_data)
            }
            Err(e) => Err(e),
        }
        .map_err(|e| {
            crate::log::http_failure("device authorization", &e);
            format!("Device authorization request failed: {e}")
        })?;

        let mut info: DeviceAuthInfo = resp
            .into_json()
            .map_err(|e| format!("Failed to parse device auth response: {e}"))?;

        // Ensure verificationUri has https:// scheme for xdg-open compatibility
        if !info.verification_uri.starts_with("http://")
            && !info.verification_uri.starts_with("https://")
        {
            info.verification_uri = format!("https://{}", info.verification_uri);
        }

        Ok(info)
    }

    pub fn poll_token_once(&self, device_code: &str) -> PollResult {
        if let Err(error) = self.check_credentials() {
            return PollResult::Error(error);
        }
        let mut form_data = vec![
            ("client_id", self.client_id.as_str()),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("scope", "r_usr w_usr"),
        ];
        if let Some(ref secret) = self.client_secret {
            form_data.push(("client_secret", secret.as_str()));
        }

        crate::log::write("API POST /v1/oauth2/token");
        match ureq::post(TOKEN_URL)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_form(&form_data)
        {
            Ok(resp) => match resp.into_json::<RawTokenResponse>() {
                Ok(raw) => {
                    let now = current_unix_timestamp();
                    let expires_at = raw.expires_in.map(|exp| now + exp);
                    let user_id = raw
                        .user_id
                        .or_else(|| raw.user.as_ref().and_then(|u| u.user_id));
                    let country_code = raw
                        .country_code
                        .or_else(|| raw.user.and_then(|u| u.country_code));

                    let session = Session {
                        access_token: raw.access_token,
                        refresh_token: raw.refresh_token,
                        user_id,
                        expires_in: raw.expires_in,
                        expires_at,
                        country_code,
                    };
                    PollResult::Success(session)
                }
                Err(e) => PollResult::Error(format!("Failed to parse token response: {e}")),
            },
            Err(ureq::Error::Status(status_code, resp)) => {
                crate::log::write(&format!(
                    "API POST /v1/oauth2/token failed: HTTP {status_code}"
                ));
                if let Ok(err_resp) = resp.into_json::<OAuthErrorResponse>() {
                    let err_type = err_resp.error.as_deref().unwrap_or("");
                    match err_type {
                        "authorization_pending" => PollResult::Pending,
                        "slow_down" => PollResult::SlowDown,
                        "expired_token" => PollResult::Expired,
                        "access_denied" => PollResult::Denied,
                        _ => PollResult::Error(format!(
                            "OAuth error '{}': {} (HTTP {})",
                            err_type,
                            err_resp.error_description.unwrap_or_default(),
                            status_code
                        )),
                    }
                } else {
                    PollResult::Error(format!("HTTP error {status_code}"))
                }
            }
            Err(ureq::Error::Transport(e)) => PollResult::Error(format!("Transport error: {e}")),
        }
    }

    pub fn poll_token(
        &self,
        device_code: &str,
        mut interval: u64,
        expires_in: u64,
    ) -> Result<Session, String> {
        self.check_credentials()?;
        if interval == 0 {
            interval = 5;
        }
        let start = Instant::now();
        let timeout = Duration::from_secs(expires_in);

        while start.elapsed() < timeout {
            std::thread::sleep(Duration::from_secs(interval));

            match self.poll_token_once(device_code) {
                PollResult::Success(session) => return Ok(session),
                PollResult::Pending => continue,
                PollResult::SlowDown => {
                    interval += 5;
                    continue;
                }
                PollResult::Expired => return Err("Device authorization code expired".to_string()),
                PollResult::Denied => return Err("User denied authorization".to_string()),
                PollResult::Error(err) => {
                    crate::log::write("Warning during device authorization polling");
                    eprintln!("  [!] Warning during token poll: {err}");
                }
            }
        }

        Err("Device authorization timed out".to_string())
    }

    pub fn refresh_session(&self, refresh_token: &str) -> Result<Session, String> {
        self.check_credentials()?;
        let mut form_data = vec![
            ("client_id", self.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", "r_usr w_usr"),
        ];
        if let Some(ref secret) = self.client_secret {
            form_data.push(("client_secret", secret.as_str()));
        }

        crate::log::write("API POST /v1/oauth2/token (refresh)");
        let resp = ureq::post(TOKEN_URL)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_form(&form_data)
            .map_err(|e| {
                crate::log::http_failure("/v1/oauth2/token", &e);
                format!("Token refresh request failed: {e}")
            })?;

        let raw: RawTokenResponse = resp
            .into_json()
            .map_err(|e| format!("Failed to parse refresh token response: {e}"))?;

        let now = current_unix_timestamp();
        let expires_at = raw.expires_in.map(|exp| now + exp);
        let previous = self.load_session();
        let user_id = raw
            .user_id
            .or_else(|| raw.user.as_ref().and_then(|u| u.user_id))
            .or_else(|| previous.as_ref().and_then(|s| s.user_id));
        let country_code = raw
            .country_code
            .or_else(|| raw.user.and_then(|u| u.country_code))
            .or_else(|| previous.and_then(|s| s.country_code));

        let new_session = Session {
            access_token: raw.access_token,
            refresh_token: raw
                .refresh_token
                .or_else(|| Some(refresh_token.to_string())),
            user_id,
            expires_in: raw.expires_in,
            expires_at,
            country_code,
        };

        self.save_session(&new_session)
            .map_err(|e| format!("Failed to save refreshed session: {e}"))?;

        Ok(new_session)
    }

    pub fn get_valid_session(&self) -> Result<Session, String> {
        let session = self
            .load_session()
            .ok_or_else(|| "No active session found".to_string())?;

        if self.is_token_expired(&session) {
            if let Some(ref refresh_tok) = session.refresh_token {
                crate::log::write("Session expired; refreshing authentication");
                self.refresh_session(refresh_tok)
            } else {
                Err("Session expired and no refresh token available to refresh".to_string())
            }
        } else {
            Ok(session)
        }
    }

    pub fn get_api_session(&self) -> Result<Session, String> {
        self.get_api_session_with(crate::api::TidalApiClient::session_country)
    }

    fn get_api_session_with(
        &self,
        fetch_country: impl FnOnce(String) -> Result<String, String>,
    ) -> Result<Session, String> {
        let mut session = self.get_valid_session()?;
        let mut cache = self
            .session_country
            .lock()
            .map_err(|_| "Failed to lock session country cache")?;
        let country = match cache.as_ref() {
            Some((token, country)) if token == &session.access_token => country.clone(),
            _ => {
                let country = fetch_country(session.access_token.clone())?;
                session.country_code = Some(country.clone());
                self.save_session(&session)
                    .map_err(|e| format!("Failed to cache session country: {e}"))?;
                *cache = Some((session.access_token.clone(), country.clone()));
                country
            }
        };
        session.country_code = Some(country);
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_client_credentials_work_without_environment() {
        let secret = std::str::from_utf8(DEFAULT_AUTH_TOKEN_BYTES).unwrap();
        let auth = AuthManager::with_credentials(DEFAULT_CLIENT_ID.to_string(), Some(secret.into()));
        assert!(auth.check_credentials().is_ok());
        assert!(AuthManager::new(None).check_credentials().is_ok());

        for secret in [None, Some(String::new()), Some("  ".to_string())] {
            let auth = AuthManager::with_credentials(DEFAULT_CLIENT_ID.to_string(), secret);
            assert!(auth.request_device_code().unwrap_err().contains("No client secret"));
            assert!(auth.refresh_session("test-refresh").unwrap_err().contains("No client secret"));
            assert!(matches!(auth.poll_token_once("test-device"), PollResult::Error(_)));
            assert!(auth.poll_token("test-device", 1, 1).is_err());
        }
        let auth = AuthManager::with_credentials(
            DEFAULT_CLIENT_ID.to_string(),
            Some("test-secret".to_string()),
        );
        assert!(auth.check_credentials().is_ok());
        let public_client = AuthManager::with_credentials("custom-public-client".to_string(), None);
        assert!(public_client.check_credentials().is_ok());
    }

    #[test]
    fn test_device_auth_info_deserialization() {
        let json = r#"{
            "deviceCode": "dev123456",
            "userCode": "ABCD-1234",
            "verificationUriComplete": "https://link.tidal.com/ABCD-1234",
            "expiresIn": 300,
            "interval": 5
        }"#;

        let parsed: Result<DeviceAuthInfo, _> = serde_json::from_str(json);
        assert!(parsed.is_ok());
        let info = parsed.unwrap();
        assert_eq!(info.user_code, "ABCD-1234");
        assert_eq!(info.verification_uri, "https://link.tidal.com/ABCD-1234");
        assert_eq!(info.expires_in, 300);
        assert_eq!(info.interval, 5);
    }

    #[test]
    fn test_session_serialization_roundtrip() {
        let session = Session {
            access_token: "test_access_token".to_string(),
            refresh_token: Some("test_refresh_token".to_string()),
            user_id: Some(987654321),
            expires_in: Some(3600),
            expires_at: Some(1700000000),
            country_code: Some("DK".to_string()),
        };

        let serialized = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.access_token, "test_access_token");
        assert_eq!(deserialized.user_id, Some(987654321));
        assert_eq!(deserialized.expires_at, Some(1700000000));
        assert_eq!(deserialized.country_code.as_deref(), Some("DK"));
        let legacy: Session = serde_json::from_str(r#"{"access_token":"token"}"#).unwrap();
        assert_eq!(legacy.country_code, None);
        let country: Session =
            serde_json::from_str(r#"{"access_token":"token","countryCode":"GB"}"#).unwrap();
        assert_eq!(country.country_code.as_deref(), Some("GB"));
    }

    #[test]
    fn test_session_expiry_check() {
        let auth = AuthManager::new(None);
        let now = current_unix_timestamp();

        let valid_session = Session {
            access_token: "valid".to_string(),
            refresh_token: None,
            user_id: None,
            expires_in: Some(3600),
            expires_at: Some(now + 1000),
            country_code: None,
        };
        assert!(!auth.is_token_expired(&valid_session));

        let expired_session = Session {
            access_token: "expired".to_string(),
            refresh_token: None,
            user_id: None,
            expires_in: Some(3600),
            expires_at: Some(now - 10),
            country_code: None,
        };
        assert!(auth.is_token_expired(&expired_session));

        // Grace period test: expires in 30 seconds should be treated as expired
        let almost_expired = Session {
            access_token: "almost".to_string(),
            refresh_token: None,
            user_id: None,
            expires_in: Some(3600),
            expires_at: Some(now + 30),
            country_code: None,
        };
        assert!(auth.is_token_expired(&almost_expired));
    }

    #[test]
    fn test_session_file_save_and_load() {
        let temp_dir =
            std::env::temp_dir().join(format!("tidal_test_{}", current_unix_timestamp()));
        let temp_file = temp_dir.join("session.json");
        std::env::set_var("TIDAL_SESSION_PATH", temp_file.to_str().unwrap());

        let auth = AuthManager::new(None);
        assert!(!auth.is_authenticated());

        let session = Session {
            access_token: "token_123".to_string(),
            refresh_token: Some("refresh_456".to_string()),
            user_id: Some(42),
            expires_in: Some(3600),
            expires_at: Some(current_unix_timestamp() + 3600),
            country_code: None,
        };

        assert!(auth.save_session(&session).is_ok());
        assert!(auth.is_authenticated());

        #[cfg(unix)]
        {
            let metadata = fs::metadata(&temp_file).unwrap();
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Session file should have 0600 permissions");
        }

        let loaded = auth.load_session().expect("Failed to load saved session");
        assert_eq!(loaded.access_token, "token_123");
        assert_eq!(loaded.user_id, Some(42));

        let mut session = session;
        session.country_code = Some("US".into());
        auth.save_session(&session).unwrap();
        assert_eq!(
            auth.get_api_session_with(|token| {
                assert_eq!(token, "token_123");
                Ok("DK".into())
            })
            .unwrap()
            .country_code
            .as_deref(),
            Some("DK")
        );
        assert_eq!(
            auth.load_session().unwrap().country_code.as_deref(),
            Some("DK")
        );
        assert_eq!(
            auth.get_api_session_with(|_| panic!("Country should be cached"))
                .unwrap()
                .country_code
                .as_deref(),
            Some("DK")
        );
        session.access_token = "new-token".into();
        auth.save_session(&session).unwrap();
        assert!(auth
            .get_api_session_with(|_| Err("country lookup failed".into()))
            .is_err());
        assert_eq!(
            auth.get_api_session_with(|_| Ok("GB".into()))
                .unwrap()
                .country_code
                .as_deref(),
            Some("GB")
        );

        assert!(auth.delete_session().is_ok());
        assert!(!auth.is_authenticated());
        assert!(auth.load_session().is_none());

        let _ = fs::remove_dir_all(&temp_dir);
        std::env::remove_var("TIDAL_SESSION_PATH");
    }

    #[test]
    fn test_live_device_authorization_request() {
        // Verifies real handshake with Tidal's Device Authorization endpoint
        let auth = AuthManager::new(None);
        let res = auth.request_device_code();
        assert!(
            res.is_ok(),
            "Live request_device_code should succeed: {:?}",
            res.err()
        );
        let info = res.unwrap();
        assert!(!info.device_code.is_empty());
        assert!(!info.user_code.is_empty());
        assert!(info.verification_uri.contains("link.tidal.com"));
        assert!(info.expires_in > 0);
        assert!(info.interval > 0);
    }
}
