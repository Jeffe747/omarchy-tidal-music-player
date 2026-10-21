use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::Read;

const API_BASE: &str = "https://api.tidal.com/v1";
pub(crate) const MAX_JSON_RESPONSE_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_ARTWORK_BYTES: u64 = 512 * 1024;
const MAX_API_TEXT_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ERROR_BODY_BYTES: u64 = 64 * 1024;

pub(crate) fn read_bounded_bytes(
    response: ureq::Response,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let mut reader = response.into_reader().take(max_bytes + 1);
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read HTTP response body: {e}"))?;
    if buf.len() as u64 > max_bytes {
        return Err(format!(
            "HTTP response body exceeds {max_bytes}-byte safety limit"
        ));
    }
    Ok(buf)
}

pub(crate) fn read_bounded_string(
    response: ureq::Response,
    max_bytes: u64,
) -> Result<String, String> {
    let buf = read_bounded_bytes(response, max_bytes)?;
    String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8 in HTTP response body: {e}"))
}

pub(crate) fn read_bounded_json<T: DeserializeOwned>(
    response: ureq::Response,
    max_bytes: u64,
) -> Result<T, String> {
    let body = read_bounded_string(response, max_bytes)?;
    serde_json::from_str(&body).map_err(|e| format!("{e}"))
}

struct ApiFailure {
    status: Option<u16>,
    quality_related: bool,
    retryable: bool,
    message: String,
}

fn transient_transport(kind: ureq::ErrorKind) -> bool {
    matches!(
        kind,
        ureq::ErrorKind::Dns
            | ureq::ErrorKind::ConnectionFailed
            | ureq::ErrorKind::Io
            | ureq::ErrorKind::ProxyConnect
    )
}

impl ApiFailure {
    fn ipc_message(&self) -> String {
        if self.retryable {
            format!("{} (retryable)", self.message)
        } else {
            self.message.clone()
        }
    }
    fn from_body(status: u16, body: &str, token: &str) -> Self {
        let body: serde_json::Value = match serde_json::from_str(body) {
            Ok(body) => body,
            Err(_) => {
                return Self {
                    status: Some(status),
                    quality_related: false,
                    retryable: matches!(status, 408 | 429 | 500..=599),
                    message: format!("HTTP {status}; error body is not valid Tidal JSON"),
                };
            }
        };
        let user_message = body["userMessage"].as_str().unwrap_or("");
        let lower = user_message.to_ascii_lowercase();
        let quality_related = body["subStatus"].as_i64() == Some(4005)
            || (matches!(status, 401 | 403 | 404)
                && [
                    "asset is not ready",
                    "not ready for playback",
                    "quality",
                    "lossless",
                    "hires",
                    "hi-res",
                    "hi res",
                ]
                .iter()
                .any(|term| lower.contains(term)));
        let mut message = format!("HTTP {status}");
        if let Some(sub_status) = body["subStatus"].as_i64() {
            message.push_str(&format!("; subStatus={sub_status}"));
        }
        if !user_message.is_empty() {
            let user_message = if token.is_empty() {
                user_message.to_string()
            } else {
                user_message.replace(token, "[redacted]")
            };
            message.push_str(&format!(
                "; userMessage={}",
                crate::log::safe_text(&user_message)
            ));
        }
        Self {
            status: Some(status),
            quality_related,
            retryable: matches!(status, 408 | 429 | 500..=599),
            message,
        }
    }
}

fn playback_url(track_id: u64, legacy: bool) -> String {
    let endpoint = if legacy {
        "playbackinfo"
    } else {
        "playbackinfopostpaywall"
    };
    format!("{API_BASE}/tracks/{track_id}/{endpoint}")
}

pub(crate) fn favorite_url(user_id: u64, track_id: u64, favorite: bool) -> String {
    if favorite {
        format!("{API_BASE}/users/{user_id}/favorites/tracks")
    } else {
        format!("{API_BASE}/users/{user_id}/favorites/tracks/{track_id}")
    }
}

pub(crate) fn album_tracks_url(album_id: u64) -> String {
    format!("{API_BASE}/albums/{album_id}/tracks")
}

pub(crate) fn artist_top_tracks_url(artist_id: u64) -> String {
    format!("{API_BASE}/artists/{artist_id}/toptracks")
}

fn playback_with_fallback(
    quality: &str,
    mut request: impl FnMut(bool, &str) -> Result<PlaybackInfoResponse, ApiFailure>,
) -> Result<PlaybackInfoResponse, String> {
    let ladder: &[&str] = match quality {
        "HI_RES_LOSSLESS" => &["HI_RES_LOSSLESS", "LOSSLESS", "HIGH", "LOW"],
        "LOSSLESS" => &["LOSSLESS", "HIGH", "LOW"],
        "HIGH" => &["HIGH", "LOW"],
        "LOW" => &["LOW"],
        _ => &["LOSSLESS", "HIGH", "LOW"],
    };
    let qualities = ladder;
    let mut index = 0;
    loop {
        let requested = qualities[index];
        crate::log::write(&format!(
            "Playback attempt: playbackinfopostpaywall quality={requested}"
        ));
        let mut result = request(false, requested);
        if let Err(error) = &result {
            crate::log::write(&format!("Playback attempt failed: {}", error.message));
            if matches!(error.status, Some(401 | 404)) || error.quality_related {
                crate::log::write(&format!(
                    "Playback retry: legacy playbackinfo quality={requested}"
                ));
                result = request(true, requested);
            }
        }
        match result {
            Ok(info) => return Ok(info),
            Err(error) => {
                crate::log::write(&format!("Playback attempt failed: {}", error.message));
                let retryable =
                    error.quality_related || matches!(error.status, Some(401 | 403 | 404));
                if !retryable || index + 1 == qualities.len() {
                    return Err(format!(
                        "Playback info request failed: {}",
                        error.ipc_message()
                    ));
                }
                crate::log::write(&format!(
                    "Playback quality rejected; retrying {}",
                    qualities[index + 1]
                ));
                index += 1;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackItem {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub duration: u64,
    #[serde(default, rename = "audioQuality")]
    pub audio_quality: Option<String>,
    #[serde(default)]
    pub artist: Option<ArtistSummary>,
    #[serde(default)]
    pub artists: Vec<ArtistSummary>,
    #[serde(default)]
    pub album: Option<AlbumSummary>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtistSummary {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumSummary {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub uuid: String,
    #[serde(default, deserialize_with = "null_string")]
    pub title: String,
    #[serde(default, deserialize_with = "null_string")]
    pub description: String,
    #[serde(default, rename = "numberOfTracks", deserialize_with = "null_u64")]
    pub number_of_tracks: u64,
    #[serde(default, deserialize_with = "null_u64")]
    pub duration: u64,
    #[serde(default, rename = "squareImage")]
    pub square_image: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub art_url: String,
}

fn null_string<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn null_u64<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    Ok(Option::<u64>::deserialize(deserializer)?.unwrap_or_default())
}

impl Playlist {
    pub fn artwork_id(&self) -> &str {
        self.square_image
            .as_deref()
            .or(self.image.as_deref())
            .unwrap_or("")
    }
    pub fn set_art_url(&mut self) {
        self.art_url = cover_url(self.artwork_id());
    }
}

#[derive(Deserialize)]
struct PlaylistPage {
    #[serde(default)]
    items: Vec<Playlist>,
}

pub fn parse_playlists(json: &str) -> Result<Vec<Playlist>, String> {
    let mut page: PlaylistPage =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse playlists: {e}"))?;
    for playlist in &mut page.items {
        playlist.set_art_url();
    }
    Ok(page.items)
}

pub fn parse_created_playlist(json: &str) -> Result<Playlist, String> {
    let mut playlist: Playlist =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse created playlist: {e}"))?;
    playlist.set_art_url();
    Ok(playlist)
}

pub fn parse_playlist_tracks(json: &str) -> Result<Vec<TrackItem>, String> {
    #[derive(Deserialize)]
    struct Page {
        #[serde(default)]
        items: Vec<TrackItem>,
    }
    let page: Page =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse playlist tracks: {e}"))?;
    Ok(page.items)
}

impl TrackItem {
    pub fn artist_name(&self) -> &str {
        self.artist
            .as_ref()
            .filter(|artist| !artist.name.is_empty())
            .or_else(|| self.artists.first())
            .map(|artist| artist.name.as_str())
            .unwrap_or("")
    }

    pub fn album_title(&self) -> &str {
        self.album
            .as_ref()
            .map(|album| album.title.as_str())
            .unwrap_or("")
    }

    pub fn cover(&self) -> &str {
        self.album
            .as_ref()
            .and_then(|album| album.cover.as_deref())
            .unwrap_or("")
    }
}

pub fn is_valid_cover_id(hash: &str) -> bool {
    !hash.is_empty()
        && hash.len() <= 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub fn cover_url(hash: &str) -> String {
    if !is_valid_cover_id(hash) {
        return String::new();
    }
    format!(
        "https://resources.tidal.com/images/{}/640x640.jpg",
        hash.replace('-', "/")
    )
}

pub fn cover_cache_dir() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("TIDAL_ART_CACHE_DIR") {
        return std::path::PathBuf::from(path);
    }
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/state")
        });
    state_dir.join("tidal/covers")
}

pub fn cached_cover_file_url(hash: &str) -> Option<String> {
    if !is_valid_cover_id(hash) {
        return None;
    }
    let path = cover_cache_dir().join(format!("{hash}.jpg"));
    if path.is_file() {
        Some(format!("file://{}", path.display()))
    } else {
        None
    }
}

pub fn fetch_and_cache_cover(hash: &str) -> Result<String, String> {
    if !is_valid_cover_id(hash) {
        return Err("Invalid cover identifier".to_string());
    }
    if let Some(url) = cached_cover_file_url(hash) {
        return Ok(url);
    }
    let dir = cover_cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cover cache dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }

    let url = cover_url(hash);
    let response = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Artwork download failed: {e}"))?;
    let bytes = read_bounded_bytes(response, MAX_ARTWORK_BYTES)?;
    // Verify JPEG (FF D8 FF) or PNG (89 50 4E 47) magic header
    let is_valid_image = (bytes.len() >= 3 && bytes[0..3] == [0xFF, 0xD8, 0xFF])
        || (bytes.len() >= 4 && bytes[0..4] == [0x89, 0x50, 0x4E, 0x47]);
    if !is_valid_image {
        return Err("Artwork response is not a valid JPEG or PNG image".to_string());
    }

    let dest = dir.join(format!("{hash}.jpg"));
    let tmp = dir.join(format!("{hash}.jpg.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("Failed to write cached cover: {e}"))?;
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Failed to finalize cached cover: {e}")
    })?;
    Ok(format!("file://{}", dest.display()))
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    tracks: Option<SearchPage>,
}

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default)]
    items: Vec<TrackItem>,
}

pub fn parse_search_results(json: &str) -> Result<Vec<TrackItem>, String> {
    let parsed: SearchResponse =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse search response: {e}"))?;
    Ok(parsed.tracks.map(|page| page.items).unwrap_or_default())
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct FavoritesPage {
    #[serde(rename = "totalNumberOfItems", default)]
    total_number_of_items: Option<u64>,
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    limit: Option<u64>,
    #[serde(default)]
    items: Vec<FavoriteEntry>,
}

#[derive(Deserialize)]
struct FavoriteEntry {
    item: TrackItem,
}

#[allow(dead_code)]
pub fn parse_favorites(json: &str) -> Result<Vec<TrackItem>, String> {
    let (tracks, _, _) = parse_favorites_page(json)?;
    Ok(tracks)
}

pub fn parse_favorites_page(json: &str) -> Result<(Vec<TrackItem>, usize, Option<u64>), String> {
    let page: FavoritesPage =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse favorites: {e}"))?;
    let count = page.items.len();
    let total = page.total_number_of_items;
    let tracks: Vec<TrackItem> = page.items.into_iter().map(|entry| entry.item).collect();
    Ok((tracks, count, total))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackInfoResponse {
    #[serde(rename = "trackId")]
    pub track_id: u64,
    #[serde(rename = "audioQuality")]
    pub audio_quality: String,
    #[serde(rename = "manifestMimeType")]
    pub manifest_mime_type: String,
    pub manifest: String, // base64 encoded
}

pub struct TidalApiClient {
    access_token: String,
    country_code: String,
}

impl TidalApiClient {
    pub fn new(access_token: String, country_code: String) -> Self {
        Self {
            access_token,
            country_code,
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    fn request(&self, request: ureq::Request, path: &str) -> Result<ureq::Response, ApiFailure> {
        crate::log::write(&format!("API GET {path}"));
        request
            .timeout(std::time::Duration::from_secs(10))
            .set("Authorization", &self.auth_header())
            .call()
            .map_err(|error| {
                let mut failure = match error {
                    ureq::Error::Status(status, response) => {
                        match read_bounded_string(response, MAX_ERROR_BODY_BYTES) {
                            Ok(body) => ApiFailure::from_body(status, &body, &self.access_token),
                            Err(_) => ApiFailure {
                                status: Some(status),
                                quality_related: false,
                                retryable: matches!(status, 408 | 429 | 500..=599),
                                message: format!("HTTP {status}; failed to read error body"),
                            },
                        }
                    }
                    ureq::Error::Transport(error) => ApiFailure {
                        status: None,
                        quality_related: false,
                        retryable: transient_transport(error.kind()),
                        message: format!(
                            "{}: HTTP transport failure ({:?}); request may be retried",
                            if transient_transport(error.kind()) {
                                "Transient network error"
                            } else {
                                "Network request error"
                            },
                            error.kind()
                        ),
                    },
                };
                crate::log::write(&format!("API GET {path} failed: {}", failure.message));
                failure.message = format!("{path}: {}", failure.message);
                failure
            })
    }

    fn request_with_retry(
        &self,
        path: &str,
        max_attempts: usize,
        mut build_req: impl FnMut() -> ureq::Request,
    ) -> Result<ureq::Response, ApiFailure> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let req = build_req();
            match self.request(req, path) {
                Ok(response) => return Ok(response),
                Err(error) => {
                    if error.retryable && attempt < max_attempts {
                        crate::log::write(&format!(
                            "API GET {path} transient error ({}); retrying attempt {}/{}",
                            error.message,
                            attempt + 1,
                            max_attempts
                        ));
                        std::thread::sleep(std::time::Duration::from_millis(150 * attempt as u64));
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    pub fn session_country(access_token: String) -> Result<String, String> {
        let client = Self::new(access_token, String::new());
        let response = client
            .request_with_retry("/v1/sessions", 3, || {
                ureq::get(&format!("{API_BASE}/sessions"))
            })
            .map_err(|error| format!("Session country request failed: {}", error.message))?;
        let value: serde_json::Value = read_bounded_json(response, MAX_JSON_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to parse session country: {e}"))?;
        parse_session_country(&value)
    }

    pub fn get_favorites_page(
        &self,
        user_id: u64,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<TrackItem>, usize, Option<u64>), String> {
        let url = format!("{API_BASE}/users/{user_id}/favorites/tracks");
        let path = format!("/v1/users/{user_id}/favorites/tracks");
        let response = self
            .request_with_retry(&path, 3, || {
                ureq::get(&url)
                    .query("countryCode", &self.country_code)
                    .query("limit", &limit.to_string())
                    .query("offset", &offset.to_string())
                    .query("order", "DATE")
                    .query("orderDirection", "DESC")
            })
            .map_err(|e| format!("Favorites request failed: {}", e.ipc_message()))?;
        let json = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read favorites: {e}"))?;
        parse_favorites_page(&json)
    }

    #[allow(dead_code)]
    pub fn get_favorites(&self, user_id: u64) -> Result<Vec<TrackItem>, String> {
        let mut offset: u64 = 0;
        let limit: u64 = 100;
        let mut tracks = Vec::new();
        loop {
            let (page_tracks, count, total) = self.get_favorites_page(user_id, offset, limit)?;
            tracks.extend(page_tracks);
            offset += limit;
            if count == 0
                || offset >= 50_000
                || total.map(|total| offset >= total).unwrap_or(count < limit as usize)
            {
                return Ok(tracks);
            }
        }
    }

    pub fn get_track(&self, track_id: u64) -> Result<TrackItem, String> {
        let url = format!("{API_BASE}/tracks/{track_id}");
        let path = format!("/v1/tracks/{track_id}");
        let response = self
            .request_with_retry(&path, 3, || {
                ureq::get(&url).query("countryCode", &self.country_code)
            })
            .map_err(|e| format!("Track request failed: {}", e.ipc_message()))?;
        read_bounded_json(response, MAX_JSON_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to parse track: {e}"))
    }

    pub fn get_playlists(&self, user_id: u64) -> Result<Vec<Playlist>, String> {
        let url = format!("{API_BASE}/users/{user_id}/playlists");
        let response = self
            .request_with_retry("/v1/users/{user_id}/playlists", 3, || {
                ureq::get(&url)
                    .query("countryCode", &self.country_code)
                    .query("limit", "50")
                    .query("offset", "0")
            })
            .map_err(|e| format!("Playlists request failed: {}", e.ipc_message()))?;
        let body = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read playlists: {e}"))?;
        parse_playlists(&body)
    }

    pub fn create_playlist(
        &self,
        user_id: u64,
        title: &str,
        description: Option<&str>,
    ) -> Result<Playlist, String> {
        let url = format!("{API_BASE}/users/{user_id}/playlists");
        let mut request = ureq::post(&url)
            .query("countryCode", &self.country_code)
            .query("title", title);
        if let Some(description) = description {
            request = request.query("description", description);
        }
        let response = self
            .request(request, &format!("/v1/users/{user_id}/playlists"))
            .map_err(|e| format!("Create playlist request failed: {}", e.ipc_message()))?;
        let body = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read created playlist: {e}"))?;
        parse_created_playlist(&body)
    }

    pub fn get_playlist_tracks(&self, uuid: &str) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/playlists/{}/tracks", encode_query(uuid));
        let path = format!("/v1/playlists/{uuid}/tracks");
        let response = self
            .request_with_retry(&path, 3, || {
                ureq::get(&url)
                    .query("countryCode", &self.country_code)
                    .query("limit", "100")
                    .query("offset", "0")
            })
            .map_err(|e| format!("Playlist tracks request failed: {}", e.ipc_message()))?;
        let body = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read playlist tracks: {e}"))?;
        parse_playlist_tracks(&body)
    }

    pub fn add_playlist_track(&self, uuid: &str, track_id: u64) -> Result<(), String> {
        let url = format!("{API_BASE}/playlists/{}/tracks", encode_query(uuid));
        let request = ureq::post(&url)
            .set("If-None-Match", "*")
            .query("trackIds", &track_id.to_string())
            .query("countryCode", &self.country_code)
            .query("onArtifactNotFound", "FAIL");
        self.request(request, &format!("/v1/playlists/{uuid}/tracks"))
            .map(|_| ())
            .map_err(|e| format!("Add playlist track failed: {}", e.ipc_message()))
    }

    pub fn set_favorite(&self, user_id: u64, track_id: u64, favorite: bool) -> Result<(), String> {
        let url = favorite_url(user_id, track_id, favorite);
        let mut request = if favorite {
            ureq::post(&url).query("trackId", &track_id.to_string())
        } else {
            ureq::delete(&url)
        };
        request = request.query("countryCode", &self.country_code);
        self.request(request, &format!("/v1/users/{user_id}/favorites/tracks"))
            .map(|_| ())
            .map_err(|e| format!("Favorite update failed: {}", e.ipc_message()))
    }

    pub fn get_album_tracks(&self, album_id: u64) -> Result<Vec<TrackItem>, String> {
        let url = album_tracks_url(album_id);
        let path = format!("/v1/albums/{album_id}/tracks");
        let response = self
            .request_with_retry(&path, 3, || {
                ureq::get(&url)
                    .query("countryCode", &self.country_code)
                    .query("limit", "100")
                    .query("offset", "0")
            })
            .map_err(|e| format!("Album tracks request failed: {}", e.ipc_message()))?;
        let json = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read album tracks: {e}"))?;
        parse_playlist_tracks(&json)
    }

    pub fn get_artist_tracks(&self, artist_id: u64) -> Result<Vec<TrackItem>, String> {
        let url = artist_top_tracks_url(artist_id);
        let path = format!("/v1/artists/{artist_id}/toptracks");
        let response = self
            .request_with_retry(&path, 3, || {
                ureq::get(&url)
                    .query("countryCode", &self.country_code)
                    .query("limit", "100")
            })
            .map_err(|e| format!("Artist tracks request failed: {}", e.ipc_message()))?;
        let json = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read artist tracks: {e}"))?;
        parse_playlist_tracks(&json)
    }

    pub fn get_track_radio(&self, track_id: u64) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/tracks/{track_id}/radio");
        let path = format!("/v1/tracks/{track_id}/radio");
        let response = self
            .request_with_retry(&path, 3, || {
                ureq::get(&url)
                    .query("countryCode", &self.country_code)
                    .query("limit", "100")
                    .query("offset", "0")
            })
            .map_err(|e| format!("Track radio request failed: {}", e.ipc_message()))?;
        let body = read_bounded_string(response, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Track radio request failed: {e}"))?;
        parse_playlist_tracks(&body)
            .map_err(|e| format!("Track radio request failed: {e}"))
    }

    pub fn get_playback_info(
        &self,
        track_id: u64,
        quality: &str,
    ) -> Result<PlaybackInfoResponse, String> {
        playback_with_fallback(quality, |legacy, requested| {
            let url = playback_url(track_id, legacy);
            let path = url.trim_start_matches("https://api.tidal.com");
            let response = self.request_with_retry(path, 3, || {
                ureq::get(&url)
                    .query("audioquality", requested)
                    .query("playbackmode", "STREAM")
                    .query("assetpresentation", "FULL")
                    .query("immersiveaudio", "false")
                    .query("immersiveAudio", "false")
                    .query("countryCode", &self.country_code)
            })?;
            read_bounded_json(response, MAX_JSON_RESPONSE_BYTES).map_err(|e| ApiFailure {
                status: None,
                quality_related: false,
                retryable: false,
                message: format!("Failed to parse playback info: {e}"),
            })
        })
    }

    pub fn search(&self, query: &str) -> Result<Vec<TrackItem>, String> {
        let url = format!(
            "{API_BASE}/search?query={}&types=TRACKS%2CALBUMS%2CPLAYLISTS&limit=20&countryCode={}",
            encode_query(query),
            encode_query(&self.country_code)
        );
        let resp = self
            .request_with_retry("/v1/search", 3, || ureq::get(&url))
            .map_err(|e| format!("Search request failed: {}", e.ipc_message()))?;
        let json = read_bounded_string(resp, MAX_API_TEXT_RESPONSE_BYTES)
            .map_err(|e| format!("Failed to read search response: {e}"))?;
        parse_search_results(&json)
    }
}

fn parse_session_country(value: &serde_json::Value) -> Result<String, String> {
    let country = value["countryCode"]
        .as_str()
        .filter(|code| code.len() == 2 && code.bytes().all(|b| b.is_ascii_alphabetic()))
        .ok_or("Session response has no valid countryCode")?;
    Ok(country.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::prelude::*;

    fn playback_fixture() -> PlaybackInfoResponse {
        PlaybackInfoResponse {
            track_id: 42,
            audio_quality: "HIGH".to_string(),
            manifest_mime_type: "application/vnd.tidal.bts".to_string(),
            manifest: String::new(),
        }
    }

    #[test]
    fn transient_transport_kinds_are_retryable() {
        assert!(transient_transport(ureq::ErrorKind::Io));
        assert!(transient_transport(ureq::ErrorKind::ConnectionFailed));
        assert!(!transient_transport(ureq::ErrorKind::InvalidUrl));
        assert!(ApiFailure::from_body(503, "{}", "").retryable);
        assert!(!ApiFailure::from_body(400, "{}", "").retryable);
    }

    #[test]
    fn test_playback_url_uses_postpaywall() {
        assert_eq!(
            playback_url(42, false),
            "https://api.tidal.com/v1/tracks/42/playbackinfopostpaywall"
        );
        assert_eq!(
            playback_url(42, true),
            "https://api.tidal.com/v1/tracks/42/playbackinfo"
        );
    }

    #[test]
    fn test_search_query_encoding() {
        assert_eq!(
            encode_query("Björk & the Sugarcubes"),
            "Bj%C3%B6rk%20%26%20the%20Sugarcubes"
        );
        assert_eq!(encode_query("a+b/c"), "a%2Bb%2Fc");
    }

    #[test]
    fn favorite_routes_match_tidals_add_and_remove_endpoints() {
        assert_eq!(
            favorite_url(5040, 42, true),
            "https://api.tidal.com/v1/users/5040/favorites/tracks"
        );
        assert_eq!(
            favorite_url(5040, 42, false),
            "https://api.tidal.com/v1/users/5040/favorites/tracks/42"
        );
    }

    #[test]
    fn album_and_artist_routes_match_catalog_endpoints() {
        assert_eq!(
            album_tracks_url(12),
            "https://api.tidal.com/v1/albums/12/tracks"
        );
        assert_eq!(
            artist_top_tracks_url(34),
            "https://api.tidal.com/v1/artists/34/toptracks"
        );
    }

    #[test]
    fn test_search_response_parsing() {
        let tracks = parse_search_results(r#"{"tracks":{"items":[{"id":7,"title":"Song","duration":123,"artists":[{"name":"Artist"}],"album":{"title":"Record","cover":"aa-bb"}}]}}"#).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 7);
        assert_eq!(tracks[0].artist_name(), "Artist");
        assert_eq!(tracks[0].album_title(), "Record");
        assert_eq!(
            cover_url(tracks[0].cover()),
            "https://resources.tidal.com/images/aa/bb/640x640.jpg"
        );
    }

    #[test]
    fn test_tidal_error_body_and_redaction() {
        let failure = ApiFailure::from_body(
            404,
            r#"{"status":404,"subStatus":2001,"userMessage":"Requested LOSSLESS quality is unavailable"}"#,
            "secret-token",
        );
        assert_eq!(failure.status, Some(404));
        assert!(failure.quality_related);
        assert_eq!(
            failure.message,
            "HTTP 404; subStatus=2001; userMessage=Requested LOSSLESS quality is unavailable"
        );
        let failure = ApiFailure::from_body(
            403,
            r#"{"userMessage":"secret-token https://cdn.example/track?signature=secret Authorization: Bearer secret-token"}"#,
            "secret-token",
        );
        assert!(!failure.message.contains("secret-token"));
        assert!(!failure.message.contains("https://"));
        assert!(!failure.message.contains("Authorization"));
        assert!(!failure.quality_related);
        assert!(ApiFailure::from_body(404, "<html>secret</html>", "")
            .message
            .contains("HTTP 404"));
        assert!(!ApiFailure::from_body(500, r#"{"userMessage":"quality"}"#, "").quality_related);
    }

    #[test]
    fn test_asset_not_ready_is_quality_related() {
        let failure = ApiFailure::from_body(
            401,
            r#"{"subStatus":4005,"userMessage":"Asset is not ready for playback"}"#,
            "",
        );
        assert!(failure.quality_related);
        assert!(failure.message.contains("subStatus=4005"));
        assert!(ApiFailure::from_body(400, r#"{"subStatus":4005}"#, "").quality_related);
        for status in [401, 403, 404] {
            for message in ["Asset is not ready", "Not ready for playback"] {
                let body = serde_json::json!({"userMessage":message}).to_string();
                assert!(ApiFailure::from_body(status, &body, "").quality_related);
            }
        }
    }

    #[test]
    fn test_asset_not_ready_falls_back_to_high_and_low() {
        for (status, body) in [
            (
                401,
                r#"{"subStatus":4005,"userMessage":"Asset is not ready for playback"}"#,
            ),
            (401, r#"{"userMessage":"Unauthorized"}"#),
            (404, "<html>Not found</html>"),
        ] {
            for successful_quality in ["HIGH", "LOW"] {
                let mut calls = Vec::new();
                let info = playback_with_fallback("LOSSLESS", |legacy, quality| {
                    calls.push((legacy, quality.to_string()));
                    if quality == successful_quality {
                        let mut info = playback_fixture();
                        info.audio_quality = quality.to_string();
                        Ok(info)
                    } else {
                        Err(ApiFailure::from_body(status, body, ""))
                    }
                })
                .unwrap();
                assert_eq!(info.audio_quality, successful_quality);
                let mut expected = vec![
                    (false, "LOSSLESS".into()),
                    (true, "LOSSLESS".into()),
                    (false, "HIGH".into()),
                ];
                if successful_quality == "LOW" {
                    expected.extend([(true, "HIGH".into()), (false, "LOW".into())]);
                }
                assert_eq!(calls, expected);
            }
        }
    }

    #[test]
    fn test_catalog_quality_is_attempted_first() {
        for quality in ["HI_RES_LOSSLESS", "LOSSLESS", "HIGH", "LOW"] {
            let mut calls = Vec::new();
            playback_with_fallback(quality, |legacy, requested| {
                calls.push((legacy, requested.to_string()));
                Ok(playback_fixture())
            })
            .unwrap();
            assert_eq!(calls, [(false, quality.to_string())]);
        }
    }

    #[test]
    fn playlist_json_and_tracks_parse() {
        let playlists = parse_playlists(r#"{"items":[{"uuid":"abc-123","title":"Mix","description":"desc","numberOfTracks":2,"duration":300,"squareImage":"aa-bb"}]}"#).unwrap();
        assert_eq!(playlists[0].uuid, "abc-123");
        assert_eq!(
            playlists[0].art_url,
            "https://resources.tidal.com/images/aa/bb/640x640.jpg"
        );
        let tracks = parse_playlist_tracks(
            r#"{"items":[{"id":9,"title":"Song","artists":[{"name":"A"}]}]}"#,
        )
        .unwrap();
        assert_eq!(tracks[0].id, 9);
    }

    #[test]
    fn created_playlist_parses_and_sets_art_url() {
        let playlist = parse_created_playlist(
            r#"{"uuid":"new-123","title":"New Mix","description":null,"numberOfTracks":0,"squareImage":"aa-bb"}"#,
        )
        .unwrap();
        assert_eq!(playlist.uuid, "new-123");
        assert_eq!(playlist.title, "New Mix");
        assert_eq!(playlist.description, "");
        assert_eq!(playlist.art_url, "https://resources.tidal.com/images/aa/bb/640x640.jpg");
    }

    #[test]
    fn quality_ladders_follow_preference() {
        for (preferred, expected) in [
            (
                "HI_RES_LOSSLESS",
                vec!["HI_RES_LOSSLESS", "LOSSLESS", "HIGH", "LOW"],
            ),
            ("LOSSLESS", vec!["LOSSLESS", "HIGH", "LOW"]),
            ("HIGH", vec!["HIGH", "LOW"]),
        ] {
            let mut calls = Vec::new();
            let _ = playback_with_fallback(preferred, |legacy, q| {
                if !legacy {
                    calls.push(q.to_string());
                }
                Err(ApiFailure {
                    status: Some(404),
                    quality_related: true,
                    retryable: false,
                    message: "unavailable".into(),
                })
            });
            assert_eq!(calls, expected);
        }
    }

    #[test]
    fn test_playback_legacy_retry_once() {
        let mut calls = Vec::new();
        let result = playback_with_fallback("LOSSLESS", |legacy, quality| {
            calls.push((legacy, quality.to_string()));
            if legacy {
                Ok(playback_fixture())
            } else {
                Err(ApiFailure::from_body(
                    404,
                    r#"{"userMessage":"Not found"}"#,
                    "",
                ))
            }
        });
        assert_eq!(result.unwrap().track_id, 42);
        assert_eq!(
            calls,
            [(false, "LOSSLESS".into()), (true, "LOSSLESS".into())]
        );
    }

    #[test]
    fn test_playback_quality_retry_and_legacy_order() {
        let mut calls = Vec::new();
        let result = playback_with_fallback("LOSSLESS", |legacy, quality| {
            calls.push((legacy, quality.to_string()));
            match (legacy, quality) {
                (true, "HIGH") => Ok(playback_fixture()),
                (false, "LOSSLESS") => Err(ApiFailure::from_body(
                    404,
                    r#"{"userMessage":"LOSSLESS quality unavailable"}"#,
                    "",
                )),
                _ => Err(ApiFailure::from_body(
                    404,
                    r#"{"userMessage":"Not found"}"#,
                    "",
                )),
            }
        });
        assert_eq!(result.unwrap().audio_quality, "HIGH");
        assert_eq!(
            calls,
            [
                (false, "LOSSLESS".into()),
                (true, "LOSSLESS".into()),
                (false, "HIGH".into()),
                (true, "HIGH".into()),
            ]
        );
        for status in [401, 403, 404] {
            let mut calls = Vec::new();
            playback_with_fallback("LOSSLESS", |legacy, quality| {
                calls.push((legacy, quality.to_string()));
                if quality == "HIGH" {
                    Ok(playback_fixture())
                } else {
                    Err(ApiFailure::from_body(
                        status,
                        r#"{"userMessage":"Audio quality rejected"}"#,
                        "",
                    ))
                }
            })
            .unwrap();
            assert_eq!(calls.last(), Some(&(false, "HIGH".into())));
            assert_eq!(calls.len(), 3);
        }
    }

    #[test]
    fn test_playback_retries_are_bounded_and_quality_specific() {
        for (status, body, quality, expected) in [
            (401, r#"{"userMessage":"Not authenticated"}"#, "LOSSLESS", 6),
            (403, r#"{"userMessage":"No subscription"}"#, "LOSSLESS", 3),
            (404, r#"{"userMessage":"Track not found"}"#, "LOSSLESS", 6),
            (500, r#"{"userMessage":"quality"}"#, "LOSSLESS", 1),
            (401, r#"{"userMessage":"quality unavailable"}"#, "HIGH", 4),
            (401, r#"{"userMessage":"quality unavailable"}"#, "LOW", 2),
            (
                404,
                r#"{"userMessage":"quality unavailable"}"#,
                "LOSSLESS",
                6,
            ),
        ] {
            let mut calls = 0;
            let error = playback_with_fallback(quality, |_, _| {
                calls += 1;
                Err(ApiFailure::from_body(status, body, ""))
            })
            .unwrap_err();
            assert_eq!(calls, expected);
            assert!(error.contains(&format!("HTTP {status}")));
        }
    }

    #[test]
    fn test_playback_stops_on_non_retryable_legacy_failure() {
        for status in [400, 429, 500] {
            let mut calls = Vec::new();
            let error = playback_with_fallback("LOSSLESS", |legacy, quality| {
                calls.push((legacy, quality.to_string()));
                Err(ApiFailure::from_body(
                    if legacy { status } else { 401 },
                    r#"{"userMessage":"Request failed"}"#,
                    "",
                ))
            })
            .unwrap_err();
            assert_eq!(
                calls,
                [(false, "LOSSLESS".into()), (true, "LOSSLESS".into())]
            );
            assert!(error.contains(&format!("HTTP {status}")));
        }
        let mut calls = 0;
        let error = playback_with_fallback("LOSSLESS", |_, _| {
            calls += 1;
            Err(ApiFailure {
                status: None,
                quality_related: false,
                retryable: false,
                message: "HTTP transport failure".into(),
            })
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert!(error.contains("HTTP transport failure"));
    }

    #[test]
    fn test_session_country_has_no_us_default() {
        assert_eq!(
            parse_session_country(&serde_json::json!({"countryCode":"DK"})).unwrap(),
            "DK"
        );
        for value in [
            serde_json::json!({}),
            serde_json::json!({"countryCode":null}),
            serde_json::json!({"countryCode":""}),
            serde_json::json!({"countryCode":"USA"}),
            serde_json::json!({"countryCode":"?!"}),
        ] {
            assert!(parse_session_country(&value).is_err());
        }
    }

    #[test]
    fn test_http_error_body_is_read_without_leaking_request_url() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/test?signed=private",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            let body = r#"{"status":404,"subStatus":2001,"userMessage":"Not found"}"#;
            write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let client = TidalApiClient::new("private-token".into(), "DK".into());
        let failure = match client.request(ureq::get(&url), "/test") {
            Ok(_) => panic!("Expected HTTP failure"),
            Err(failure) => failure,
        };
        server.join().unwrap();
        assert_eq!(
            failure.message,
            "/test: HTTP 404; subStatus=2001; userMessage=Not found"
        );
        assert!(!failure.message.contains("private"));
    }

    #[test]
    fn test_request_with_retry_recovers_from_transient_failure() {
        use std::io::{BufRead, BufReader, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let request_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&request_count);

        let server = std::thread::spawn(move || {
            // First attempt: return 503 Service Unavailable (retryable)
            if let Ok((mut stream, _)) = listener.accept() {
                count_clone.fetch_add(1, Ordering::SeqCst);
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                    line.clear();
                }
                let body = r#"{"status":503,"userMessage":"Service Unavailable"}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
            // Second attempt: return 200 OK
            if let Ok((mut stream, _)) = listener.accept() {
                count_clone.fetch_add(1, Ordering::SeqCst);
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                    line.clear();
                }
                let body = r#"{"status":"ok"}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });

        let client = TidalApiClient::new("test-token".into(), "DK".into());
        let url = format!("http://{addr}/test");
        let resp = client.request_with_retry("/test", 3, || ureq::get(&url));
        assert!(resp.is_ok());
        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        server.join().unwrap();
    }

    #[test]
    fn test_request_with_retry_does_not_retry_non_transient_failure() {
        use std::io::{BufRead, BufReader, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let request_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&request_count);

        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                count_clone.fetch_add(1, Ordering::SeqCst);
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                    line.clear();
                }
                let body = r#"{"status":400,"userMessage":"Bad Request"}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });

        let client = TidalApiClient::new("test-token".into(), "DK".into());
        let url = format!("http://{addr}/bad");
        let resp = client.request_with_retry("/bad", 3, || ureq::get(&url));
        assert!(resp.is_err());
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
        server.join().unwrap();
    }

    #[test]
    fn test_parse_favorites() {
        let tracks = parse_favorites(r#"{"items":[{"created":"today","item":{"id":42,"title":"Song","duration":180,"artist":{"id":1,"name":"Artist"},"artists":[{"name":"Other"}],"album":{"id":2,"title":"Album","cover":"ab-cd-ef"}}}],"totalNumberOfItems":1}"#).unwrap();
        assert_eq!(tracks[0].id, 42);
        assert_eq!(tracks[0].title, "Song");
        assert_eq!(tracks[0].artist_name(), "Artist");
        assert_eq!(tracks[0].album_title(), "Album");
        assert_eq!(tracks[0].duration, 180);
        assert_eq!(tracks[0].cover(), "ab-cd-ef");
    }

    #[test]
    fn test_favorites_page_pagination_metadata() {
        let page: FavoritesPage = serde_json::from_str(
            r#"{"totalNumberOfItems":2128,"limit":100,"offset":100,"items":[{"item":{"id":42,"title":"Song"}}]}"#,
        )
        .unwrap();
        assert_eq!(page.total_number_of_items, Some(2128));
        assert_eq!(page.limit, Some(100));
        assert_eq!(page.offset, Some(100));
        assert_eq!(page.items.len(), 1);
    }

    #[test]
    fn test_parse_favorites_page() {
        let json = r#"{"totalNumberOfItems":1900,"limit":100,"offset":0,"items":[{"item":{"id":1,"title":"T1"}},{"item":{"id":2,"title":"T2"}}]}"#;
        let (tracks, count, total) = parse_favorites_page(json).unwrap();
        assert_eq!(count, 2);
        assert_eq!(total, Some(1900));
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].id, 1);
        assert_eq!(tracks[1].id, 2);
    }

    #[test]
    fn test_parse_favorites_missing_optional_fields() {
        let tracks = parse_favorites(r#"{"items":[{"item":{"id":1,"title":"Minimal"}},{"item":{"id":2,"title":"Fallback","artists":[{"name":"First"}],"album":{}}}]}"#).unwrap();
        assert_eq!(tracks[0].duration, 0);
        assert_eq!(tracks[0].artist_name(), "");
        assert_eq!(tracks[0].cover(), "");
        assert_eq!(tracks[1].artist_name(), "First");
        assert_eq!(tracks[1].album_title(), "");
    }

    #[test]
    fn test_parse_favorites_empty_and_invalid() {
        assert!(parse_favorites(r#"{"items":[]}"#).unwrap().is_empty());
        assert!(parse_favorites(r#"{"items":[{"item":{"title":"Missing id"}}]}"#).is_err());
    }

    #[test]
    fn test_cover_url() {
        assert_eq!(
            cover_url("ab-cd-ef"),
            "https://resources.tidal.com/images/ab/cd/ef/640x640.jpg"
        );
        assert_eq!(cover_url(""), "");
    }

    #[test]
    fn test_playback_info_resolves_bts() {
        let data = serde_json::json!({
            "trackId":42, "audioQuality":"LOSSLESS",
            "manifestMimeType":"application/vnd.tidal.bts",
            "manifest":BASE64_STANDARD.encode(r#"{"encryptionType":"NONE","urls":["https://example.com/track.flac"]}"#)
        });
        let info: PlaybackInfoResponse = serde_json::from_value(data).unwrap();
        assert_eq!(info.track_id, 42);
        assert_eq!(
            crate::playback::PlaybackEngine::parse_stream_url(
                &info.manifest_mime_type,
                &info.manifest
            )
            .unwrap(),
            "https://example.com/track.flac"
        );
    }

    #[test]
    fn test_read_bounded_json_enforces_byte_limit() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            let payload = format!("{{\"data\":\"{}\"}}", "x".repeat(2048));
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
        });

        let response = ureq::get(&format!("http://{addr}/oversized")).call().unwrap();
        let err = read_bounded_json::<serde_json::Value>(response, 512).unwrap_err();
        server.join().unwrap();
        assert!(
            err.contains("exceeds 512-byte safety limit"),
            "Unexpected error: {err}"
        );
    }

    #[test]
    fn test_is_valid_cover_id() {
        assert!(is_valid_cover_id("ab-cd-ef-0123"));
        assert!(!is_valid_cover_id(""));
        assert!(!is_valid_cover_id("../etc/passwd"));
        assert!(!is_valid_cover_id("ab/cd"));
        assert!(!is_valid_cover_id(&"a".repeat(65)));
    }

    #[test]
    fn test_read_bounded_bytes_enforces_byte_limit() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            let payload = vec![0xFFu8; 2048];
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(&payload);
        });

        let response = ureq::get(&format!("http://{addr}/oversized.jpg"))
            .call()
            .unwrap();
        let err = read_bounded_bytes(response, 1024).unwrap_err();
        server.join().unwrap();
        assert!(
            err.contains("exceeds 1024-byte safety limit"),
            "Unexpected error: {err}"
        );
    }
}
