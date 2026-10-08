use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.tidal.com/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackItem {
    pub id: u64,
    pub title: String,
    pub duration: u64,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub artist: Option<ArtistSummary>,
    pub album: Option<AlbumSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistSummary {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumSummary {
    pub id: u64,
    pub title: String,
    pub cover: Option<String>,
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

    pub fn get_playback_info(&self, track_id: u64, quality: &str) -> Result<PlaybackInfoResponse, String> {
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
