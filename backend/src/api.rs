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
    let qualities = [quality, "HIGH", "LOW"];
    let qualities = match quality {
        "HIGH" => &qualities[1..],
        "LOW" => &qualities[2..],
        _ => &qualities[..],
    };
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
                    return Err(format!("Playback info request failed: {}", error.message));
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
    let parsed: SearchResponse = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse search response: {e}"))?;
    Ok(parsed.tracks.map(|page| page.items).unwrap_or_default())
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
                    .query("immersiveaudio", "false")
                    .query("immersiveAudio", "false")
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

    pub fn search(&self, query: &str) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/search?query={}&types=TRACKS%2CALBUMS%2CPLAYLISTS&limit=20&countryCode={}", encode_query(query), encode_query(&self.country_code));
        let resp = self
            .request(ureq::get(&url), "/v1/search")
            .map_err(|e| format!("Search request failed: {}", e.message))?;
        let json = resp.into_string().map_err(|e| format!("Failed to read search response: {e}"))?;
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
        assert_eq!(encode_query("Björk & the Sugarcubes"), "Bj%C3%B6rk%20%26%20the%20Sugarcubes");
        assert_eq!(encode_query("a+b/c"), "a%2Bb%2Fc");
    }

    #[test]
    fn test_search_response_parsing() {
        let tracks = parse_search_results(r#"{"tracks":{"items":[{"id":7,"title":"Song","duration":123,"artists":[{"name":"Artist"}],"album":{"title":"Record","cover":"aa-bb"}}]}}"#).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 7);
        assert_eq!(tracks[0].artist_name(), "Artist");
        assert_eq!(tracks[0].album_title(), "Record");
        assert_eq!(cover_url(tracks[0].cover()), "https://resources.tidal.com/images/aa/bb/640x640.jpg");
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
