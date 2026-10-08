use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.tidal.com/v1";

struct ApiFailure {
    status: Option<u16>,
    quality_related: bool,
    message: String,
}

impl ApiFailure {
    fn from_body(status: u16, body: &str, token: &str) -> Self {
        let body: serde_json::Value = match serde_json::from_str(body) {
            Ok(body) => body,
            Err(_) => {
                return Self {
                    status: Some(status),
                    quality_related: false,
                    message: format!("HTTP {status}; error body is not valid Tidal JSON"),
                };
            }
        };
        let user_message = body["userMessage"].as_str().unwrap_or("");
        let lower = user_message.to_ascii_lowercase();
        let quality_related = matches!(status, 401 | 403 | 404)
            && ["quality", "lossless", "hires", "hi-res", "hi res"]
                .iter()
                .any(|term| lower.contains(term));
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

fn playback_with_fallback(
    quality: &str,
    mut request: impl FnMut(bool, &str) -> Result<PlaybackInfoResponse, ApiFailure>,
) -> Result<PlaybackInfoResponse, String> {
    let mut requested = quality;
    loop {
        let mut result = request(false, requested);
        let mut quality_rejected = false;
        if let Err(error) = &result {
            quality_rejected = error.quality_related;
            if error.status == Some(404) {
                crate::log::write("Playback endpoint HTTP 404; retrying legacy playbackinfo");
                result = request(true, requested);
            }
        }
        match result {
            Ok(info) => return Ok(info),
            Err(error) => {
                if requested != "HIGH" && (quality_rejected || error.quality_related) {
                    crate::log::write("Playback quality rejected; retrying HIGH");
                    requested = "HIGH";
                } else {
                    return Err(format!("Playback info request failed: {}", error.message));
                }
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

pub fn cover_url(hash: &str) -> String {
    if hash.is_empty() {
        return String::new();
    }
    format!(
        "https://resources.tidal.com/images/{}/640x640.jpg",
        hash.replace('-', "/")
    )
}

#[derive(Deserialize)]
struct FavoritesPage {
    items: Vec<FavoriteEntry>,
}

#[derive(Deserialize)]
struct FavoriteEntry {
    item: TrackItem,
}

pub fn parse_favorites(json: &str) -> Result<Vec<TrackItem>, String> {
    let page: FavoritesPage =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse favorites: {e}"))?;
    Ok(page.items.into_iter().map(|entry| entry.item).collect())
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
            .set("Authorization", &self.auth_header())
            .call()
            .map_err(|error| {
                let mut failure = match error {
                    ureq::Error::Status(status, response) => match response.into_string() {
                        Ok(body) => ApiFailure::from_body(status, &body, &self.access_token),
                        Err(_) => ApiFailure {
                            status: Some(status),
                            quality_related: false,
                            message: format!("HTTP {status}; failed to read error body"),
                        },
                    },
                    ureq::Error::Transport(error) => ApiFailure {
                        status: None,
                        quality_related: false,
                        message: format!("HTTP transport failure: {:?}", error.kind()),
                    },
                };
                crate::log::write(&format!("API GET {path} failed: {}", failure.message));
                failure.message = format!("{path}: {}", failure.message);
                failure
            })
    }

    pub fn session_country(access_token: String) -> Result<String, String> {
        let client = Self::new(access_token, String::new());
        let response = client
            .request(ureq::get(&format!("{API_BASE}/sessions")), "/v1/sessions")
            .map_err(|error| format!("Session country request failed: {}", error.message))?;
        let value: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse session country: {e}"))?;
        parse_session_country(&value)
    }

    pub fn get_favorites(&self, user_id: u64) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/users/{user_id}/favorites/tracks");
        let mut tracks = Vec::new();
        loop {
            let json = self
                .request(
                    ureq::get(&url)
                        .query("countryCode", &self.country_code)
                        .query("limit", "100")
                        .query("offset", &tracks.len().to_string())
                        .query("order", "DATE")
                        .query("orderDirection", "DESC"),
                    &format!("/v1/users/{user_id}/favorites/tracks"),
                )
                .map_err(|e| format!("Favorites request failed: {}", e.message))?
                .into_string()
                .map_err(|e| format!("Failed to read favorites: {e}"))?;
            let items = parse_favorites(&json)?;
            let count = items.len();
            tracks.extend(items);
            if count < 100 {
                return Ok(tracks);
            }
        }
    }

    pub fn get_track(&self, track_id: u64) -> Result<TrackItem, String> {
        self.request(
            ureq::get(&format!("{API_BASE}/tracks/{track_id}"))
                .query("countryCode", &self.country_code),
            &format!("/v1/tracks/{track_id}"),
        )
        .map_err(|e| format!("Track request failed: {}", e.message))?
        .into_json()
        .map_err(|e| format!("Failed to parse track: {e}"))
    }

    pub fn get_playback_info(
        &self,
        track_id: u64,
        quality: &str,
    ) -> Result<PlaybackInfoResponse, String> {
        playback_with_fallback(quality, |legacy, requested| {
            let url = playback_url(track_id, legacy);
            let response = self.request(
                ureq::get(&url)
                    .query("audioquality", requested)
                    .query("playbackmode", "STREAM")
                    .query("assetpresentation", "FULL")
                    .query("countryCode", &self.country_code),
                url.trim_start_matches("https://api.tidal.com"),
            )?;
            response.into_json().map_err(|e| ApiFailure {
                status: None,
                quality_related: false,
                message: format!("Failed to parse playback info: {e}"),
            })
        })
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/search");
        let resp = self
            .request(
                ureq::get(&url)
                    .query("query", query)
                    .query("types", "TRACKS")
                    .query("limit", "20")
                    .query("countryCode", &self.country_code),
                "/v1/search",
            )
            .map_err(|e| format!("Search request failed: {}", e.message))?;

        #[derive(Deserialize)]
        struct SearchWrapper {
            tracks: Option<TracksPage>,
        }

        #[derive(Deserialize)]
        struct TracksPage {
            items: Vec<TrackItem>,
        }

        let parsed: SearchWrapper = resp
            .into_json()
            .map_err(|e| format!("Failed to parse search response: {e}"))?;

        Ok(parsed.tracks.map(|t| t.items).unwrap_or_default())
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
            assert_eq!(calls.len(), if status == 404 { 3 } else { 2 });
        }
    }

    #[test]
    fn test_playback_retries_are_bounded_and_quality_specific() {
        for (status, body, quality, expected) in [
            (401, r#"{"userMessage":"Not authenticated"}"#, "LOSSLESS", 1),
            (403, r#"{"userMessage":"No subscription"}"#, "LOSSLESS", 1),
            (404, r#"{"userMessage":"Track not found"}"#, "LOSSLESS", 2),
            (500, r#"{"userMessage":"quality"}"#, "LOSSLESS", 1),
            (401, r#"{"userMessage":"quality unavailable"}"#, "HIGH", 1),
            (
                404,
                r#"{"userMessage":"quality unavailable"}"#,
                "LOSSLESS",
                4,
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
}
