use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.tidal.com/v1";

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
    pub fn new(access_token: String, country_code: Option<String>) -> Self {
        Self {
            access_token,
            country_code: country_code.unwrap_or_else(|| "US".to_string()),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    pub fn get_favorites(&self, user_id: u64) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/users/{user_id}/favorites/tracks");
        let mut tracks = Vec::new();
        loop {
            let json = ureq::get(&url)
                .set("Authorization", &self.auth_header())
                .query("countryCode", &self.country_code)
                .query("limit", "100")
                .query("offset", &tracks.len().to_string())
                .query("order", "DATE")
                .query("orderDirection", "DESC")
                .call()
                .map_err(|e| format!("Favorites request failed: {e}"))?
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
        ureq::get(&format!("{API_BASE}/tracks/{track_id}"))
            .set("Authorization", &self.auth_header())
            .query("countryCode", &self.country_code)
            .call()
            .map_err(|e| format!("Track request failed: {e}"))?
            .into_json()
            .map_err(|e| format!("Failed to parse track: {e}"))
    }

    pub fn get_playback_info(
        &self,
        track_id: u64,
        quality: &str,
    ) -> Result<PlaybackInfoResponse, String> {
        let url = format!("{API_BASE}/tracks/{track_id}/playbackinfo");
        let resp = ureq::get(&url)
            .set("Authorization", &self.auth_header())
            .query("audioquality", quality)
            .query("playbackmode", "STREAM")
            .query("assetpresentation", "FULL")
            .query("countryCode", &self.country_code)
            .call()
            .map_err(|e| format!("Playback info request failed: {e}"))?;

        resp.into_json::<PlaybackInfoResponse>()
            .map_err(|e| format!("Failed to parse playback info: {e}"))
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str) -> Result<Vec<TrackItem>, String> {
        let url = format!("{API_BASE}/search");
        let resp = ureq::get(&url)
            .set("Authorization", &self.auth_header())
            .query("query", query)
            .query("types", "TRACKS")
            .query("limit", "20")
            .query("countryCode", &self.country_code)
            .call()
            .map_err(|e| format!("Search request failed: {e}"))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::prelude::*;

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
