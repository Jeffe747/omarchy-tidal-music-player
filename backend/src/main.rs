mod api;
mod auth;
mod ipc;
mod log;
mod mpris;
mod playback;
pub mod cast;
#[cfg(test)]
#[path = "tests/ui_endpoints.rs"]
mod ui_endpoint_tests;

use api::{cover_url, Playlist, TidalApiClient, TrackItem};
use auth::AuthManager;
use ipc::{FavoriteTrack, IpcCommand, IpcServer, IpcStateMessage, PlayerMessage};
use playback::PlaybackEngine;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub(crate) static SHUTDOWN: AtomicBool = AtomicBool::new(false);
pub(crate) static FAVORITES_LOADING: AtomicBool = AtomicBool::new(false);
pub(crate) static PLAY_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

extern "C" fn request_shutdown(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

pub(crate) struct Player {
    engine: PlaybackEngine,
    favorites: Vec<TrackItem>,
    queue: Vec<TrackItem>,
    playlists: Vec<Playlist>,
    current_playlist_id: Option<String>,
    current: Option<TrackItem>,
    quality: String,
    preferred_quality: String,
    eof_handled_track: Option<u64>,
    track_has_started: bool,
    shuffle: bool,
    repeat_mode: String,
    cast_active: bool,
    cast_device_name: Option<String>,
    cast_state: crate::cast::CastState,
    cast_volume: f64,
    cast_session: Option<crate::cast::client::CastSession>,
    stream_proxy: Option<Arc<crate::cast::proxy::StreamProxy>>,
    cast_discovery: Arc<crate::cast::discovery::CastDiscovery>,
    cast_firewall_blocked: bool,
    last_firewall_check: std::time::Instant,
}

impl Player {
    fn new() -> Self {
        let stream_proxy = match crate::cast::proxy::StreamProxy::new() {
            Ok(proxy) => {
                let arc = Arc::new(proxy);
                Arc::clone(&arc).start();
                Some(arc)
            }
            Err(e) => {
                log::write(&format!("Failed to start Cast audio relay proxy: {e}"));
                None
            }
        };
        let cast_discovery = Arc::new(crate::cast::discovery::CastDiscovery::new());
        let proxy_port = stream_proxy
            .as_ref()
            .map(|p| p.port())
            .unwrap_or(crate::cast::proxy::StreamProxy::DEFAULT_PORT);
        let cast_firewall_blocked = crate::cast::proxy::check_cast_ports_blocked(proxy_port);

        Self {
            engine: PlaybackEngine::new(),
            favorites: Vec::new(),
            queue: Vec::new(),
            playlists: Vec::new(),
            current_playlist_id: None,
            current: None,
            quality: load_quality_preference(),
            preferred_quality: load_quality_preference(),
            eof_handled_track: None,
            track_has_started: false,
            shuffle: false,
            repeat_mode: "off".into(),
            cast_active: false,
            cast_device_name: None,
            cast_state: crate::cast::CastState::Idle,
            cast_volume: 1.0,
            cast_session: None,
            stream_proxy,
            cast_discovery,
            cast_firewall_blocked,
            last_firewall_check: std::time::Instant::now(),
        }
    }

    fn update_firewall_check(&mut self) {
        let proxy_port = self
            .stream_proxy
            .as_ref()
            .map(|p| p.port())
            .unwrap_or(crate::cast::proxy::StreamProxy::DEFAULT_PORT);
        self.cast_firewall_blocked = crate::cast::proxy::check_cast_ports_blocked(proxy_port);
        self.last_firewall_check = std::time::Instant::now();
    }
}

fn settings_path() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("TIDAL_SETTINGS_PATH") {
        return std::path::PathBuf::from(path);
    }
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/state")
        });
    let legacy = state_dir.join("omarchy/tidal/settings.json");
    let standard = state_dir.join("tidal/settings.json");
    if legacy.exists() && !standard.exists() {
        legacy
    } else {
        standard
    }
}
fn load_quality_preference() -> String {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["audio_quality"].as_str().map(str::to_owned))
        .filter(|q| ["HI_RES_LOSSLESS", "LOSSLESS", "HIGH"].contains(&q.as_str()))
        .unwrap_or_else(|| "LOSSLESS".into())
}
fn save_quality_preference(quality: &str) -> Result<(), String> {
    let path = settings_path();
    let parent = path.parent().ok_or("Invalid settings path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({"audio_quality": quality}))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn run_cli_auth(auth_manager: &AuthManager) -> Result<(), String> {
    println!("==========================================================");
    println!("  Omarchy Tidal Music Player - Device Authentication");
    println!("==========================================================");
    println!("Requesting device authorization code from Tidal...");

    let info = auth_manager.request_device_code()?;

    println!();
    println!("  --> User Code:        {}", info.user_code);
    println!("  --> Verification URI: {}", info.verification_uri);
    println!("  --> Expires in:       {} seconds", info.expires_in);
    println!();
    println!("Opening browser to verify your account...");

    // Launch xdg-open if available
    let _ = std::process::Command::new("xdg-open")
        .arg(&info.verification_uri)
        .spawn();

    println!("Waiting for confirmation on link.tidal.com...");
    let session = auth_manager.poll_token(&info.device_code, info.interval, info.expires_in)?;

    auth_manager
        .save_session(&session)
        .map_err(|e| format!("Failed to save session: {e}"))?;

    println!();
    println!("==========================================================");
    println!("  [✓] Authentication successful!");
    println!("  [✓] User ID: {:?}", session.user_id);
    println!(
        "  [✓] Session saved to: {}",
        AuthManager::session_file_path().display()
    );
    println!("==========================================================");

    Ok(())
}

fn build_status_message(auth_manager: &AuthManager, player: &Player) -> IpcStateMessage {
    let authenticated = match auth_manager.get_valid_session() {
        Ok(s) => !s.access_token.is_empty(),
        Err(_) => false,
    };

    let mut playing = false;
    let mut position = Some(0.0);
    if player.cast_active {
        playing = player.cast_state == crate::cast::CastState::Playing
            || player.cast_state == crate::cast::CastState::Buffering;
        position = player
            .cast_session
            .as_ref()
            .map(|s| s.position())
            .or(Some(0.0));
    } else if player.current.is_some() {
        let state = (|| {
            let idle = player
                .engine
                .property("idle-active")?
                .as_bool()
                .ok_or("mpv returned invalid idle state")?;
            let paused = player
                .engine
                .property("pause")?
                .as_bool()
                .ok_or("mpv returned invalid pause state")?;
            Ok::<bool, String>(!idle && !paused)
        })();
        match state {
            Ok(active) => playing = active,
            Err(error) => log::write(&format!("Failed to read playback status: {error}")),
        }
        position = player.engine.position().ok().or(Some(0.0));
    }
    let track = player.current.as_ref();
    IpcStateMessage {
        msg_type: "status".to_string(),
        daemon_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        authenticated,
        is_playing: playing,
        track_id: track.map(|t| t.id),
        track_title: track.map(|t| t.title.clone()),
        track_artist: track.map(|t| t.artist_name().to_string()),
        track_album: track.map(|t| t.album_title().to_string()),
        track_art_url: track.map(|t| cover_url(t.cover())),
        duration: Some(track.map(|t| t.duration as f64).unwrap_or(0.0)),
        position,
        audio_quality: Some(player.quality.clone()),
        preferred_audio_quality: Some(player.preferred_quality.clone()),
        shuffle: player.shuffle,
        repeat_mode: player.repeat_mode.clone(),
        is_favorite: track.is_some_and(|t| player.favorites.iter().any(|f| f.id == t.id)),
        artist_id: track.and_then(|t| {
            t.artist
                .as_ref()
                .map(|a| a.id)
                .or_else(|| t.artists.first().map(|a| a.id))
        }),
        album_id: track.and_then(|t| t.album.as_ref().map(|a| a.id)),
        cast_active: player.cast_active,
        cast_device_name: player.cast_device_name.clone(),
        cast_state: player.cast_state.as_str().to_string(),
        cast_volume: player
            .cast_session
            .as_ref()
            .map(|s| s.volume())
            .unwrap_or(player.cast_volume),
        cast_firewall_blocked: player.cast_firewall_blocked,
    }
}

pub(crate) fn broadcast_status(auth: &AuthManager, player: &Mutex<Player>, ipc: &IpcServer) {
    match player.lock() {
        Ok(mut player) => {
            if player.last_firewall_check.elapsed() > std::time::Duration::from_secs(10) {
                player.update_firewall_check();
            }
            ipc.broadcast(&build_status_message(auth, &player))
        }
        Err(error) => log::write(&format!("Failed to lock player: {error}")),
    }
}

fn api_client(auth: &AuthManager) -> Result<(TidalApiClient, Option<u64>), String> {
    let session = auth.get_api_session()?;
    Ok((
        TidalApiClient::new(
            session.access_token,
            session.country_code.ok_or("Session country is missing")?,
        ),
        session.user_id,
    ))
}

fn api_call<T>(
    auth: &AuthManager,
    mut call: impl FnMut(&TidalApiClient, Option<u64>) -> Result<T, String>,
) -> Result<T, String> {
    let (api, user_id) = api_client(auth)?;
    match call(&api, user_id) {
        Ok(value) => Ok(value),
        Err(error) if error.contains("HTTP 401") => {
            let session = auth.load_session().ok_or(error.clone())?;
            let refresh = session.refresh_token.as_deref().ok_or(error.clone())?;
            log::write("API returned HTTP 401; refreshing saved session and retrying once");
            auth.refresh_session(refresh).map_err(|refresh_error| {
                format!("{error}; token refresh failed: {refresh_error}")
            })?;
            let (api, user_id) = api_client(auth)?;
            call(&api, user_id)
        }
        Err(error) => Err(error),
    }
}

fn run_cli_probe(
    auth: &AuthManager,
    id: u64,
    quality_override: Option<&str>,
) -> Result<(), String> {
    let track = api_call(auth, |api, _| api.get_track(id))?;
    println!("Track ID: {}", track.id);
    println!("Title: {}", track.title);
    println!("Artist: {}", track.artist_name());
    println!("Album: {}", track.album_title());
    println!("Duration: {} seconds", track.duration);
    println!(
        "Catalog Audio Quality: {}",
        track.audio_quality.as_deref().unwrap_or("Unknown")
    );

    let requested_quality = quality_override
        .map(str::to_owned)
        .unwrap_or_else(load_quality_preference);
    println!("Requested Audio Quality: {requested_quality}");
    let info = api_call(auth, |api, _| api.get_playback_info(id, &requested_quality))?;
    println!("audioQuality: {}", info.audio_quality);
    println!("manifestMimeType: {}", info.manifest_mime_type);
    let mut engine = PlaybackEngine::new();
    let url = engine.resolve_stream_url(&info.manifest_mime_type, &info.manifest)?;
    let preview: String = url.chars().take(60).collect();
    println!(
        "Resolved Stream URL: {preview}{}",
        if preview.len() < url.len() { "..." } else { "" }
    );
    engine.stop()?;
    println!("[✓] Probe successful! Stream is playable.");
    Ok(())
}

fn load_favorites_async(
    auth: Arc<AuthManager>,
    player: Arc<Mutex<Player>>,
    ipc: IpcServer,
) {
    if FAVORITES_LOADING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            // 1. Fetch first batch (offset 0, limit 100) immediately
            let (first_tracks, count, total) = api_call(&auth, |api, user_id| {
                let uid = user_id.ok_or("Session has no user ID; please sign in again")?;
                api.get_favorites_page(uid, 0, 100)
            })?;
            let has_more = total.map(|t| (count as u64) < t).unwrap_or(count == 100);

            {
                let mut p = player.lock().map_err(|e| e.to_string())?;
                p.favorites = first_tracks.clone();
                if p.current_playlist_id.is_none() {
                    p.queue = p.favorites.clone();
                }
            }

            ipc.broadcast(&PlayerMessage::FavoritesLoaded {
                tracks: first_tracks.iter().map(FavoriteTrack::from).collect(),
                offset: Some(0),
                total,
                has_more: Some(has_more),
            });

            if !has_more {
                ipc.broadcast(&PlayerMessage::FavoritesComplete { total: count });
                return Ok(());
            }

            // 2. Fetch subsequent pages progressively in background
            let mut offset: u64 = 100;
            let limit: u64 = 100;
            let mut total_loaded = count;

            while !SHUTDOWN.load(Ordering::Relaxed) {
                if !auth.is_authenticated() {
                    return Ok(());
                }
                let (page_tracks, page_count, _) = api_call(&auth, |api, user_id| {
                    let uid = user_id.ok_or("Session has no user ID; please sign in again")?;
                    api.get_favorites_page(uid, offset, limit)
                })?;
                if !auth.is_authenticated() || page_count == 0 {
                    break;
                }
                total_loaded += page_count;

                {
                    let mut p = player.lock().map_err(|e| e.to_string())?;
                    p.favorites.extend(page_tracks.clone());
                    if p.current_playlist_id.is_none() {
                        p.queue.extend(page_tracks.clone());
                    }
                }

                ipc.broadcast(&PlayerMessage::FavoritesProgress {
                    loaded: total_loaded,
                    total,
                    tracks: page_tracks.iter().map(FavoriteTrack::from).collect(),
                });

                offset += limit;
                if total.map(|t| offset >= t).unwrap_or(page_count < limit as usize)
                    || offset >= 50_000
                {
                    break;
                }
            }

            if auth.is_authenticated() {
                ipc.broadcast(&PlayerMessage::FavoritesComplete {
                    total: total_loaded,
                });
            }
            Ok(())
        })();

        FAVORITES_LOADING.store(false, Ordering::SeqCst);

        if let Err(error) = result {
            log::write(&format!("Async favorites loading failed: {error}"));
            ipc.broadcast(&PlayerMessage::FavoritesError { error: &error });
        }
    });
}

fn load_playlists(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
) -> Result<(), String> {
    let playlists = api_call(auth, |api, user_id| {
        api.get_playlists(user_id.ok_or("Session has no user ID; please sign in again")?)
    })?;
    player.lock().map_err(|e| e.to_string())?.playlists = playlists.clone();
    ipc.broadcast(&PlayerMessage::PlaylistsLoaded { playlists });
    Ok(())
}

fn create_playlist(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    title: Option<String>,
    description: Option<String>,
) -> Result<(), String> {
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or("title is required")?;
    let playlist = api_call(auth, |api, user_id| {
        api.create_playlist(
            user_id.ok_or("Session has no user ID; please sign in again")?,
            title,
            description.as_deref(),
        )
    })?;
    let mut state = player.lock().map_err(|e| e.to_string())?;
    state.playlists.insert(0, playlist);
    ipc.broadcast(&PlayerMessage::PlaylistsLoaded {
        playlists: state.playlists.clone(),
    });
    Ok(())
}

fn load_playlist_tracks(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    playlist_id: String,
) -> Result<(), String> {
    let tracks = api_call(auth, |api, _| api.get_playlist_tracks(&playlist_id))?;
    let mut state = player.lock().map_err(|e| e.to_string())?;
    state.queue = tracks.clone();
    state.current_playlist_id = Some(playlist_id.clone());
    ipc.broadcast(&PlayerMessage::PlaylistTracksLoaded {
        playlist_id: &playlist_id,
        tracks: tracks.iter().map(FavoriteTrack::from).collect(),
    });
    Ok(())
}

fn toggle_favorite(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    target_track_id: Option<u64>,
) -> Result<(), String> {
    let (known_track, was_favorite) = {
        let state = player.lock().map_err(|e| e.to_string())?;
        let track = if let Some(id) = target_track_id {
            state
                .favorites
                .iter()
                .find(|item| item.id == id)
                .cloned()
                .or_else(|| state.queue.iter().find(|item| item.id == id).cloned())
                .or_else(|| state.current.as_ref().filter(|item| item.id == id).cloned())
        } else {
            state.current.clone()
        };
        let favorite = target_track_id
            .and_then(|id| state.favorites.iter().find(|t| t.id == id).map(|_| id))
            .is_some()
            || track
                .as_ref()
                .is_some_and(|item| state.favorites.iter().any(|fav| fav.id == item.id));
        (track, favorite)
    };
    let track = if let Some(track) = known_track {
        track
    } else if let Some(id) = target_track_id {
        api_call(auth, |api, _| api.get_track(id))?
    } else {
        return Err("No current track".to_string());
    };
    api_call(auth, |api, user_id| {
        api.set_favorite(
            user_id.ok_or("Session has no user ID")?,
            track.id,
            !was_favorite,
        )
    })?;
    let (is_fav, track_id) = {
        let mut state = player.lock().map_err(|e| e.to_string())?;
        if was_favorite {
            state.favorites.retain(|item| item.id != track.id);
            if state.current_playlist_id.is_none() {
                state.queue.retain(|item| item.id != track.id);
            }
        } else {
            state.favorites.push(track.clone());
            if state.current_playlist_id.is_none() {
                state.queue.push(track.clone());
            }
        }
        (!was_favorite, track.id)
    };
    ipc.broadcast(&PlayerMessage::FavoriteUpdated {
        track_id,
        is_favorite: is_fav,
        track: is_fav.then(|| FavoriteTrack::from(&track)),
    });
    let state = player.lock().map_err(|e| e.to_string())?;
    ipc.broadcast(&build_status_message(auth, &state));
    Ok(())
}

fn add_playlist_track(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    playlist_id: String,
    track_id: u64,
) -> Result<(), String> {
    api_call(auth, |api, _| {
        api.add_playlist_track(&playlist_id, track_id)
    })?;
    let current = player
        .lock()
        .map_err(|e| e.to_string())?
        .current_playlist_id
        .clone();
    if current.as_deref() == Some(playlist_id.as_str()) {
        load_playlist_tracks(auth, player, ipc, playlist_id)?;
    }
    Ok(())
}

fn explore(auth: &AuthManager, ipc: &IpcServer, view: &str, id: u64) -> Result<(), String> {
    let tracks = api_call(auth, |api, _| {
        if view == "album" {
            api.get_album_tracks(id)
        } else if view == "radio" {
            api.get_track_radio(id)
        } else {
            api.get_artist_tracks(id)
        }
    })?;
    ipc.broadcast(&PlayerMessage::ExplorationLoaded {
        view,
        tracks: tracks.iter().map(FavoriteTrack::from).collect(),
    });
    Ok(())
}

fn search_catalog(
    auth: &AuthManager,
    ipc: &IpcServer,
    query: Option<String>,
    request_id: u64,
) -> Result<(), String> {
    let query = query.unwrap_or_default();
    if query.trim().is_empty() {
        ipc.broadcast(&PlayerMessage::SearchResults {
            request_id,
            results: Vec::new(),
        });
        return Ok(());
    }
    let tracks = api_call(auth, |api, _| api.search(query.trim()))?;
    ipc.broadcast(&PlayerMessage::SearchResults {
        request_id,
        results: tracks.iter().map(FavoriteTrack::from).collect(),
    });
    Ok(())
}

fn play_track(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    track_id: Option<u64>,
) -> Result<(), String> {
    let id = track_id.ok_or("play_track requires track_id")?;
    let req_id = PLAY_REQUEST_ID.fetch_add(1, Ordering::SeqCst) + 1;
    let (cached_track, preferred_quality, is_casting) = {
        let p = player.lock().map_err(|e| e.to_string())?;
        let track = p
            .queue
            .iter()
            .chain(p.favorites.iter())
            .find(|track| track.id == id)
            .cloned();
        (track, p.preferred_quality.clone(), p.cast_active)
    };
    let track = match cached_track {
        Some(track) => track,
        None => api_call(auth, |api, _| api.get_track(id))?,
    };
    log::write(&format!(
        "Playback track: id={id}; title={}; artist={}; catalog audio quality={:?}",
        track.title,
        track.artist_name(),
        track.audio_quality
    ));
    let requested_quality = if is_casting && preferred_quality == "HI_RES_LOSSLESS" {
        "LOSSLESS".to_string()
    } else {
        preferred_quality.clone()
    };
    let info = api_call(auth, |api, _| api.get_playback_info(id, &requested_quality))?;
    let mime = match info.manifest_mime_type.as_str() {
        "application/vnd.tidal.bts" | "application/dash+xml" => info.manifest_mime_type.as_str(),
        _ => "unsupported",
    };
    log::write(&format!("Playback manifest MIME: {mime}"));

    let mut player = player.lock().map_err(|e| e.to_string())?;
    if req_id != PLAY_REQUEST_ID.load(Ordering::SeqCst) {
        return Ok(());
    }
    if !player.queue.iter().any(|item| item.id == id) {
        if player.favorites.iter().any(|item| item.id == id) {
            player.queue = player.favorites.clone();
            player.current_playlist_id = None;
        } else {
            player.queue = vec![track.clone()];
            player.current_playlist_id = None;
        }
    }
    let (direct_url, stream_mime) =
        PlaybackEngine::parse_stream_url_and_mime(&info.manifest_mime_type, &info.manifest)?;
    if let Some(session) = player.cast_session.clone().filter(|_| player.cast_active) {
        let stream_url = if let Some(ref proxy) = player.stream_proxy {
            proxy.register_stream(id, direct_url.clone(), stream_mime.clone(), &session.device.ip)
        } else {
            direct_url
        };
        let _ = player.engine.set_pause(true);
        if let Err(e) = session.load_track(
            stream_url,
            stream_mime,
            track.title.clone(),
            track.artist_name().to_string(),
            track.album_title().to_string(),
            cover_url(track.cover()),
            0.0,
        ) {
            log::write(&format!("Cast load_track failed: {e}"));
            return Err(e);
        }
        player.cast_state = crate::cast::CastState::Buffering;
    } else {
        let url = player
            .engine
            .resolve_stream_url(&info.manifest_mime_type, &info.manifest)?;
        match player.engine.load_url(&url) {
            Ok(()) => log::write("mpv load result: success"),
            Err(error) => {
                log::write(&format!("mpv load result: failed: {error}"));
                return Err(error);
            }
        }
    }
    player.current = Some(track);
    player.quality = info.audio_quality;
    player.eof_handled_track = None;
    player.track_has_started = false;
    ipc.broadcast(&PlayerMessage::PlaybackStarted { track_id: id });
    ipc.broadcast(&build_status_message(auth, &player));
    Ok(())
}

pub(crate) fn play_or_resume(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    toggle: bool,
) -> Result<(), String> {
    let first_favorite = {
        let state = player.lock().map_err(|e| e.to_string())?;
        if state.current.is_some() {
            None
        } else {
            state
                .queue
                .first()
                .or_else(|| state.favorites.first())
                .map(|track| track.id)
        }
    };
    if let Some(id) = first_favorite {
        return play_track(auth, player, ipc, Some(id));
    }
    let no_current = player.lock().map_err(|e| e.to_string())?.current.is_none();
    if no_current {
        let first = {
            let mut waited = 0;
            loop {
                let state = player.lock().map_err(|e| e.to_string())?;
                if let Some(track) = state.queue.first().or_else(|| state.favorites.first()) {
                    break Some(track.id);
                }
                drop(state);
                if !FAVORITES_LOADING.load(Ordering::SeqCst) || waited >= 10 {
                    break None;
                }
                thread::sleep(std::time::Duration::from_millis(100));
                waited += 1;
            }
        };
        if let Some(id) = first {
            return play_track(auth, player, ipc, Some(id));
        }
        let (first_tracks, count, _) = api_call(auth, |api, user_id| {
            api.get_favorites_page(
                user_id.ok_or("Session has no user ID; please sign in again")?,
                0,
                100,
            )
        })?;
        if count > 0 {
            let first_id = first_tracks[0].id;
            let mut state = player.lock().map_err(|e| e.to_string())?;
            state.favorites = first_tracks.clone();
            if state.current_playlist_id.is_none() {
                state.queue = first_tracks;
            }
            drop(state);
            return play_track(auth, player, ipc, Some(first_id));
        }
        return Err("No favorite tracks are available".to_string());
    }
    let state = player.lock().map_err(|e| e.to_string())?;
    if state.cast_active {
        if let Some(ref session) = state.cast_session {
            if toggle {
                if state.cast_state == crate::cast::CastState::Playing {
                    let _ = session.pause();
                } else {
                    let _ = session.play();
                }
            } else {
                let _ = session.play();
            }
        }
        return Ok(());
    }
    if toggle {
        state.engine.toggle_pause()
    } else {
        state.engine.set_pause(false)
    }
}

pub(crate) fn navigate(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    forward: bool,
) -> Result<(), String> {
    let target = {
        let state = player.lock().map_err(|e| e.to_string())?;
        let current = state.current.as_ref().ok_or("No current track")?;
        let pos = if state.cast_active {
            state.cast_session.as_ref().map(|s| s.position()).unwrap_or(0.0)
        } else {
            state.engine.position().unwrap_or(0.0)
        };
        if !forward && pos > 3.0 {
            if state.cast_active {
                if let Some(ref s) = state.cast_session {
                    let _ = s.seek(0.0);
                }
            } else {
                state.engine.seek(0.0)?;
            }
            drop(state);
            broadcast_status(auth, player, ipc);
            return Ok(());
        }
        let index = state
            .queue
            .iter()
            .position(|t| t.id == current.id)
            .ok_or("Current track is not in active queue")?;
        let len = state.queue.len();
        if len == 0 {
            return Err("Active queue is empty".to_string());
        }
        if forward && state.repeat_mode == "one" {
            index
        } else if forward && state.shuffle && len > 1 {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let mut target = seed % len;
            if target == index {
                target = (target + 1) % len;
            }
            target
        } else if forward {
            if state.repeat_mode == "off" && index + 1 == len {
                return Err("End of queue".into());
            }
            (index + 1) % len
        } else {
            (index + len - 1) % len
        }
    };
    let id = player.lock().map_err(|e| e.to_string())?.queue[target].id;
    play_track(auth, player, ipc, Some(id))
}

fn connect_cast(
    auth: &Arc<AuthManager>,
    player: &Arc<Mutex<Player>>,
    ipc: &IpcServer,
    device_id: Option<String>,
) -> Result<(), String> {
    let target_id = device_id.ok_or_else(|| "device_id is required".to_string())?;

    let (cached_dev, discovery) = {
        let p = player.lock().map_err(|e| e.to_string())?;
        let dev = p
            .cast_discovery
            .get_devices()
            .into_iter()
            .find(|d| d.id == target_id);
        (dev, Arc::clone(&p.cast_discovery))
    };

    let device = match cached_dev {
        Some(d) => d,
        None => {
            let scanned = discovery.discover(Duration::from_millis(800));
            scanned
                .into_iter()
                .find(|d| d.id == target_id)
                .ok_or_else(|| format!("Cast device not found: {target_id}"))?
        }
    };

    let (prev_session, was_playing, current_pos, current_track, proxy, preferred_quality) = {
        let mut p = player.lock().map_err(|e| e.to_string())?;
        let (prev, was_playing, pos) = if p.cast_active {
            let pos = p.cast_session.as_ref().map(|s| s.position()).unwrap_or(0.0);
            let was_playing = p.cast_state == crate::cast::CastState::Playing;
            (p.cast_session.take(), was_playing, pos)
        } else {
            let was_playing = p
                .engine
                .property("pause")
                .ok()
                .and_then(|v| v.as_bool())
                .map(|paused| !paused)
                .unwrap_or(false);
            let pos = p.engine.position().ok().unwrap_or(0.0);
            (None, was_playing, pos)
        };
        let track = p.current.clone();

        // Pause local mpv audio
        let _ = p.engine.set_pause(true);
        p.cast_state = crate::cast::CastState::Connecting;
        let proxy = match p.stream_proxy.as_ref() {
            Some(prx) => Arc::clone(prx),
            None => {
                p.cast_state = crate::cast::CastState::Idle;
                return Err("Cast stream proxy is unavailable on this system".to_string());
            }
        };
        let quality = if p.preferred_quality == "HI_RES_LOSSLESS" {
            "LOSSLESS".to_string()
        } else {
            p.preferred_quality.clone()
        };
        (prev, was_playing, pos, track, proxy, quality)
    };

    if let Some(prev) = prev_session {
        proxy.clear_streams();
        prev.disconnect();
    }

    broadcast_status(auth, player, ipc);

    let auth_cb = Arc::clone(auth);
    let player_cb = Arc::clone(player);
    let ipc_cb = ipc.clone();
    let on_track_finished = Arc::new(move || {
        let auth_t = Arc::clone(&auth_cb);
        let player_t = Arc::clone(&player_cb);
        let ipc_t = ipc_cb.clone();
        thread::spawn(move || {
            if let Err(e) = navigate(&auth_t, &player_t, &ipc_t, true) {
                log::write(&format!("Cast auto-advance error: {e}"));
            }
        });
    });

    let session = match crate::cast::client::CastSession::connect(device.clone(), on_track_finished) {
        Ok(s) => s,
        Err(e) => {
            log::write(&format!("Cast connect error: {e}"));
            if let Ok(mut p) = player.lock() {
                p.cast_state = crate::cast::CastState::Idle;
                if was_playing {
                    let _ = p.engine.set_pause(false);
                }
            }
            broadcast_status(auth, player, ipc);
            return Err(e);
        }
    };

    // If currently playing a track, transfer playback smoothly to the Chromecast
    if let Some(track) = current_track {
        if let Ok(info) = api_call(auth, |api, _| api.get_playback_info(track.id, &preferred_quality)) {
            if let Ok((direct_url, stream_mime)) = PlaybackEngine::parse_stream_url_and_mime(&info.manifest_mime_type, &info.manifest) {
                let proxy_url = proxy.register_stream(track.id, direct_url.clone(), stream_mime.clone(), &device.ip);
                let _ = session.load_track(
                    proxy_url,
                    stream_mime,
                    track.title.clone(),
                    track.artist_name().to_string(),
                    track.album_title().to_string(),
                    cover_url(track.cover()),
                    if was_playing { current_pos } else { 0.0 },
                );
            }
        }
    }

    if let Ok(mut p) = player.lock() {
        p.cast_active = true;
        p.cast_device_name = Some(device.name.clone());
        p.cast_state = session.state();
        p.cast_volume = session.volume();
        p.cast_session = Some(session);
    }

    broadcast_status(auth, player, ipc);
    Ok(())
}

fn disconnect_cast(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
) -> Result<(), String> {
    let (session, proxy) = {
        let mut p = player.lock().map_err(|e| e.to_string())?;
        let _ = p.engine.set_pause(true);
        p.cast_active = false;
        p.cast_device_name = None;
        p.cast_state = crate::cast::CastState::Idle;
        let proxy = p.stream_proxy.clone();
        (p.cast_session.take(), proxy)
    };

    if let Some(proxy) = proxy {
        proxy.clear_streams();
    }

    if let Some(session) = session {
        session.disconnect();
    }

    broadcast_status(auth, player, ipc);
    Ok(())
}

fn start_position_ticker(auth: Arc<AuthManager>, player: Arc<Mutex<Player>>, ipc: IpcServer) {
    thread::spawn(move || {
        while !SHUTDOWN.load(Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_millis(250));
            let (position, duration, is_playing, eof_track, status_changed) = match player.lock() {
                Ok(mut state) => {
                    if state.cast_active {
                        let mut status_changed = false;
                        let session_opt = state.cast_session.clone();
                        if let Some(session) = session_opt {
                            if !session.is_active() {
                                state.cast_active = false;
                                state.cast_device_name = None;
                                state.cast_state = crate::cast::CastState::Idle;
                                state.cast_session = None;
                                if let Some(ref proxy) = state.stream_proxy {
                                    proxy.clear_streams();
                                }
                                status_changed = true;
                                (None, None, false, None, status_changed)
                            } else {
                                let session_state = session.state();
                                if session_state != state.cast_state {
                                    state.cast_state = session_state;
                                    status_changed = true;
                                }
                                let session_vol = session.volume();
                                if (session_vol - state.cast_volume).abs() > 0.005 {
                                    state.cast_volume = session_vol;
                                    status_changed = true;
                                }
                                let pos = session.position();
                                let dur = state.current.as_ref().map(|t| t.duration as f64);
                                let playing = state.cast_state == crate::cast::CastState::Playing
                                    || state.cast_state == crate::cast::CastState::Buffering;
                                (Some(pos), dur, playing, None, status_changed)
                            }
                        } else {
                            state.cast_active = false;
                            state.cast_state = crate::cast::CastState::Idle;
                            (None, None, false, None, true)
                        }
                    } else {
                        let track_id = state.current.as_ref().map(|t| t.id);
                        if let Some(id) = track_id {
                            let idle_active = state
                                .engine
                                .property("idle-active")
                                .ok()
                                .and_then(|value| value.as_bool())
                                .unwrap_or(true);
                            let eof_reached = state.engine.eof_reached().unwrap_or(false);

                            if !idle_active {
                                state.track_has_started = true;
                            }

                            if state.track_has_started
                                && (eof_reached || idle_active)
                                && state.eof_handled_track != Some(id)
                            {
                                state.eof_handled_track = Some(id);
                                state.track_has_started = false;
                                (None, None, false, Some(id), false)
                            } else {
                                let playing = !idle_active
                                    && !state
                                        .engine
                                        .property("pause")
                                        .ok()
                                        .and_then(|value| value.as_bool())
                                        .unwrap_or(true);
                                (
                                    state.engine.position().ok(),
                                    state.engine.duration().ok(),
                                    playing,
                                    None,
                                    false,
                                )
                            }
                        } else {
                            (None, None, false, None, false)
                        }
                    }
                }
                Err(_) => continue,
            };
            if status_changed {
                broadcast_status(&auth, &player, &ipc);
            }
            if let Some(id) = eof_track {
                let result = navigate(&auth, &player, &ipc, true);
                if let Err(error) = result {
                    log::write(&format!("Auto-advance failed for {id}: {error}"));
                    ipc.broadcast(&PlayerMessage::PlaybackError { error: &error });
                }
                continue;
            }
            if is_playing {
                if let Some(position) = position {
                    ipc.broadcast(&serde_json::json!({"type":"position_changed", "position":position, "duration":duration}));
                }
            }
        }
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let auth_manager = Arc::new(AuthManager::new(None));

    if let Some(index) = args.iter().position(|a| a == "--probe") {
        let result = (|| {
            if !(3..=4).contains(&args.len()) || index != 1 {
                return Err(
                    "Usage: tidal-daemon --probe <track_id> [HI_RES_LOSSLESS|LOSSLESS|HIGH]"
                        .to_string(),
                );
            }
            let id = args[2]
                .parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or("track_id must be a positive integer")?;
            let quality = args.get(3).map(String::as_str);
            if let Some(value) = quality {
                if !["HI_RES_LOSSLESS", "LOSSLESS", "HIGH"].contains(&value) {
                    return Err(
                        "Usage: tidal-daemon --probe <track_id> [HI_RES_LOSSLESS|LOSSLESS|HIGH]"
                            .to_string(),
                    );
                }
            }
            run_cli_probe(&auth_manager, id, quality)
        })();
        if let Err(error) = result {
            eprintln!("[FAIL] Probe error: {error}");
            std::process::exit(1);
        }
        return;
    }

    if args.iter().any(|a| a == "--test-auth" || a == "--login") {
        match run_cli_auth(&auth_manager) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("[FAIL] Authentication error: {e}");
                std::process::exit(1);
            }
        }
    }

    if args.iter().any(|a| a == "--status") {
        match auth_manager.load_session() {
            Some(session) => {
                let expired = auth_manager.is_token_expired(&session);
                println!("Authenticated: true");
                println!("User ID: {:?}", session.user_id);
                println!("Token Expired: {expired}");
                println!(
                    "Session File: {}",
                    AuthManager::session_file_path().display()
                );
                std::process::exit(0);
            }
            None => {
                println!("Authenticated: false");
                println!(
                    "Session File: {}",
                    AuthManager::session_file_path().display()
                );
                std::process::exit(0);
            }
        }
    }

    if args.iter().any(|a| a == "--logout") {
        let _ = auth_manager.delete_session();
        println!("Logged out successfully.");
        std::process::exit(0);
    }

    if args.iter().any(|a| a == "--discover-cast") {
        println!("Scanning LAN for Google Cast devices via mDNS...");
        let discovery = cast::discovery::CastDiscovery::new();
        let devices = discovery.discover(std::time::Duration::from_millis(1500));
        if devices.is_empty() {
            println!("No Google Cast devices found.");
        } else {
            println!("Found {} Cast device(s):", devices.len());
            for d in &devices {
                println!(
                    "  - [{}] {} ({}) at {}:{}",
                    d.id, d.name, d.model, d.ip, d.port
                );
            }
        }
        std::process::exit(0);
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Omarchy Tidal Music Player Daemon (tidal-daemon)");
        println!("Usage: tidal-daemon [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --test-auth, --login  Run OAuth 2.0 Device Flow authorization CLI runner");
        println!("  --status              Print current session and authentication status");
        println!("  --logout              Clear stored session credentials");
        println!("  --discover-cast       Scan LAN for Google Cast devices via mDNS");
        println!(
            "  --probe <track_id> [QUALITY]  Resolve a track's stream (optional quality override)"
        );
        println!("  --help, -h            Print this help message");
        std::process::exit(0);
    }

    if let Err(error) = log::init() {
        eprintln!("Failed to initialize private daemon log: {error}");
        std::process::exit(1);
    }
    log::write("Starting Omarchy Tidal Daemon");
    let player = Arc::new(Mutex::new(Player::new()));
    let ipc_server = IpcServer::new();
    mpris::start(
        Arc::clone(&auth_manager),
        Arc::clone(&player),
        ipc_server.clone(),
    );
    start_position_ticker(
        Arc::clone(&auth_manager),
        Arc::clone(&player),
        ipc_server.clone(),
    );

    // Check existing session
    match auth_manager.get_valid_session() {
        Ok(session) => {
            log::write(&format!(
                "Authenticated session found for User ID: {:?}",
                session.user_id
            ));
        }
        Err(_) => {
            log::write("No active session found. Ready for pairing via Quickshell");
        }
    }

    let listener = match ipc_server.bind() {
        Ok(l) => l,
        Err(e) => {
            log::write(&format!("Failed to bind IPC socket: {e}"));
            return;
        }
    };

    log::write(&format!(
        "  [✓] Listening for Quickshell connections on {}",
        ipc_server.socket_path().display()
    ));

    let auth_in_progress = Arc::new(AtomicBool::new(false));

    // Signal handlers only set an atomic flag; cleanup runs on the main thread.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            request_shutdown as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            request_shutdown as *const () as libc::sighandler_t,
        );
    }
    if let Err(error) = listener.set_nonblocking(true) {
        log::write(&format!("Failed to configure IPC listener: {error}"));
        return;
    }
    while !SHUTDOWN.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let auth_ref = Arc::clone(&auth_manager);
                let ipc_ref = ipc_server.clone();
                let auth_flag = Arc::clone(&auth_in_progress);
                let player_ref = Arc::clone(&player);

                let write_stream = match stream.try_clone() {
                    Ok(s) => s,
                    Err(e) => {
                        log::write(&format!("Failed to clone stream: {e}"));
                        continue;
                    }
                };
                ipc_ref.register_client(write_stream);

                thread::spawn(move || {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();

                    loop {
                        match reader.read_line(&mut line) {
                            Ok(0) => break,
                            Ok(_) => {}
                            Err(error) => {
                                log::write(&format!("Failed to read IPC command: {error}"));
                                break;
                            }
                        }
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            line.clear();
                            continue;
                        }

                        if let Ok(cmd) = serde_json::from_str::<IpcCommand>(trimmed) {
                            let command = match cmd.command.as_str() {
                                "get_status"
                                | "get_auth_status"
                                | "get_favorites"
                                | "play_track"
                                | "toggle_play"
                                | "play"
                                | "pause"
                                | "seek"
                                | "next"
                                | "previous"
                                | "resume"
                                | "search"
                                | "get_playlists"
                                | "create_playlist"
                                | "add_playlist_track"
                                | "get_playlist_tracks"
                                | "set_audio_quality"
                                | "toggle_shuffle"
                                | "cycle_repeat"
                                | "toggle_favorite"
                                | "get_album_tracks"
                                | "get_artist_tracks"
                                | "get_track_radio"
                                | "discover_cast_devices"
                                | "refresh_cast_devices"
                                | "connect_cast"
                                | "disconnect_cast"
                                | "set_cast_volume"
                                | "start_auth"
                                | "logout" => cmd.command.as_str(),
                                _ => "unknown",
                            };
                            log::write(&format!("IPC command: {command}"));

                            match cmd.command.as_str() {
                                "get_status" | "get_auth_status" => {
                                    broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                }
                                "get_favorites" => {
                                    load_favorites_async(
                                        Arc::clone(&auth_ref),
                                        Arc::clone(&player_ref),
                                        ipc_ref.clone(),
                                    );
                                }
                                "search" => {
                                    let request_id = cmd.request_id.unwrap_or_default();
                                    let query = cmd.query;
                                    let auth = Arc::clone(&auth_ref);
                                    let ipc = ipc_ref.clone();
                                    thread::spawn(move || {
                                        if let Err(error) =
                                            search_catalog(&auth, &ipc, query, request_id)
                                        {
                                            log::write(&format!("Search failed: {error}"));
                                            ipc.broadcast(&PlayerMessage::SearchError {
                                                request_id,
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "get_playlists" => {
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    thread::spawn(move || {
                                        if let Err(error) =
                                            load_playlists(&auth, &player, &ipc)
                                        {
                                            ipc.broadcast(&PlayerMessage::PlaylistsError {
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "create_playlist" => {
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    let title = cmd.title;
                                    let description = cmd.description;
                                    thread::spawn(move || {
                                        if let Err(error) = create_playlist(
                                            &auth,
                                            &player,
                                            &ipc,
                                            title,
                                            description,
                                        ) {
                                            ipc.broadcast(&PlayerMessage::PlaylistsError {
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "add_playlist_track" => {
                                    let playlist_id =
                                        cmd.playlist_id.or(cmd.uuid).unwrap_or_default();
                                    let track_id = cmd.track_id;
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    thread::spawn(move || {
                                        let result = if playlist_id.is_empty() {
                                            Err("playlist_id is required".to_string())
                                        } else if let Some(track_id) = track_id {
                                            add_playlist_track(
                                                &auth,
                                                &player,
                                                &ipc,
                                                playlist_id,
                                                track_id,
                                            )
                                        } else {
                                            Err("track_id is required".to_string())
                                        };
                                        if let Err(error) = result {
                                            ipc.broadcast(&serde_json::json!({"type":"playback_error","error":error}));
                                        }
                                    });
                                }
                                "get_playlist_tracks" => {
                                    let playlist_id =
                                        cmd.playlist_id.or(cmd.uuid).unwrap_or_default();
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    thread::spawn(move || {
                                        if playlist_id.is_empty() {
                                            ipc.broadcast(&PlayerMessage::PlaylistTracksError {
                                                playlist_id: "",
                                                error: "playlist_id is required",
                                            });
                                        } else if let Err(error) = load_playlist_tracks(
                                            &auth,
                                            &player,
                                            &ipc,
                                            playlist_id.clone(),
                                        ) {
                                            ipc.broadcast(&PlayerMessage::PlaylistTracksError {
                                                playlist_id: &playlist_id,
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "set_audio_quality" => {
                                    let result = (|| {
                                        let quality =
                                            cmd.quality.as_deref().ok_or("quality is required")?;
                                        if !["HI_RES_LOSSLESS", "LOSSLESS", "HIGH"]
                                            .contains(&quality)
                                        {
                                            return Err("Unsupported audio quality".to_string());
                                        }
                                        save_quality_preference(quality)?;
                                        let mut state =
                                            player_ref.lock().map_err(|e| e.to_string())?;
                                        state.preferred_quality = quality.to_string();
                                        drop(state);
                                        broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                        Ok::<(), String>(())
                                    })();
                                    if let Err(error) = result {
                                        ipc_ref.broadcast(&serde_json::json!({"type":"playback_error","error":error}));
                                    }
                                }
                                "toggle_shuffle" => {
                                    if let Ok(mut state) = player_ref.lock() {
                                        state.shuffle = !state.shuffle;
                                    }
                                    broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                }
                                "cycle_repeat" => {
                                    if let Ok(mut state) = player_ref.lock() {
                                        state.repeat_mode = match state.repeat_mode.as_str() {
                                            "off" => "all",
                                            "all" => "one",
                                            _ => "off",
                                        }
                                        .to_string();
                                    }
                                    broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                }
                                "toggle_favorite" => {
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    let track_id = cmd.track_id;
                                    thread::spawn(move || {
                                        if let Err(error) = toggle_favorite(
                                            &auth,
                                            &player,
                                            &ipc,
                                            track_id,
                                        ) {
                                            ipc.broadcast(&PlayerMessage::PlaybackError {
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "get_album_tracks" | "get_artist_tracks" | "get_track_radio" => {
                                    let view = if cmd.command == "get_album_tracks" {
                                        "album"
                                    } else if cmd.command == "get_track_radio" {
                                        "radio"
                                    } else {
                                        "artist"
                                    };
                                    let id = cmd
                                        .album_id
                                        .or(cmd.artist_id)
                                        .or(cmd.track_id)
                                        .unwrap_or_default();
                                    let auth = Arc::clone(&auth_ref);
                                    let ipc = ipc_ref.clone();
                                    thread::spawn(move || {
                                        if let Err(error) = explore(&auth, &ipc, view, id) {
                                            ipc.broadcast(&PlayerMessage::ExplorationError {
                                                view,
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "play_track" => {
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    let track_id = cmd.track_id;
                                    thread::spawn(move || {
                                        if let Err(error) =
                                            play_track(&auth, &player, &ipc, track_id)
                                        {
                                            log::write(&format!("Playback failed: {error}"));
                                            ipc.broadcast(&PlayerMessage::PlaybackError {
                                                error: &error,
                                            });
                                        }
                                    });
                                }
                                "toggle_play" | "play" | "resume" | "pause" | "seek" => {
                                    let result = (|| {
                                        if cmd.command == "toggle_play"
                                            || cmd.command == "play"
                                            || cmd.command == "resume"
                                        {
                                            return play_or_resume(
                                                &auth_ref,
                                                &player_ref,
                                                &ipc_ref,
                                                cmd.command == "toggle_play",
                                            );
                                        }
                                        let player =
                                            player_ref.lock().map_err(|e| e.to_string())?;
                                        if player.cast_active {
                                            if let Some(ref s) = player.cast_session {
                                                match cmd.command.as_str() {
                                                    "pause" => s.pause()?,
                                                    _ => s.seek(
                                                        cmd.position.ok_or("seek requires position")?,
                                                    )?,
                                                }
                                            }
                                            return Ok(());
                                        }
                                        match cmd.command.as_str() {
                                            "pause" => player.engine.set_pause(true),
                                            _ => player.engine.seek(
                                                cmd.position.ok_or("seek requires position")?,
                                            ),
                                        }
                                    })();
                                    if let Err(error) = result {
                                        log::write(&format!("Playback control failed: {error}"));
                                        ipc_ref.broadcast(&PlayerMessage::PlaybackError {
                                            error: &error,
                                        });
                                    }
                                    broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                }
                                "next" | "previous" => {
                                    if let Err(error) = navigate(
                                        &auth_ref,
                                        &player_ref,
                                        &ipc_ref,
                                        cmd.command == "next",
                                    ) {
                                        log::write(&format!("Navigation failed: {error}"));
                                        ipc_ref.broadcast(&PlayerMessage::PlaybackError {
                                            error: &error,
                                        });
                                    }
                                }
                                "discover_cast_devices" | "refresh_cast_devices" => {
                                    let discovery = match player_ref.lock() {
                                        Ok(mut p) => {
                                            p.update_firewall_check();
                                            Arc::clone(&p.cast_discovery)
                                        }
                                        Err(e) => {
                                            log::write(&format!("Failed to lock player: {e}"));
                                            continue;
                                        }
                                    };
                                    broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                    let ipc = ipc_ref.clone();
                                    let is_refresh = cmd.command == "refresh_cast_devices";
                                    thread::spawn(move || {
                                        if !is_refresh {
                                            let cached = discovery.get_devices();
                                            if !cached.is_empty() {
                                                ipc.broadcast(&PlayerMessage::CastDevicesDiscovered {
                                                    devices: cached,
                                                });
                                                return;
                                            }
                                        }
                                        let devices =
                                            discovery.discover(Duration::from_millis(1500));
                                        ipc.broadcast(&PlayerMessage::CastDevicesDiscovered {
                                            devices,
                                        });
                                    });
                                }
                                "connect_cast" => {
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    let device_id = cmd.device_id.clone();
                                    thread::spawn(move || {
                                        if let Err(e) = connect_cast(&auth, &player, &ipc, device_id) {
                                            log::write(&format!("connect_cast error: {e}"));
                                            ipc.broadcast(&PlayerMessage::CastError { error: &e });
                                        }
                                    });
                                }
                                "disconnect_cast" => {
                                    let auth = Arc::clone(&auth_ref);
                                    let player = Arc::clone(&player_ref);
                                    let ipc = ipc_ref.clone();
                                    thread::spawn(move || {
                                        if let Err(e) = disconnect_cast(&auth, &player, &ipc) {
                                            log::write(&format!("disconnect_cast error: {e}"));
                                            ipc.broadcast(&PlayerMessage::CastError { error: &e });
                                        }
                                    });
                                }
                                "set_cast_volume" => {
                                    if let Some(vol) = cmd.volume {
                                        let clamped = vol.clamp(0.0, 1.0);
                                        if let Ok(mut p) = player_ref.lock() {
                                            p.cast_volume = clamped;
                                            if let Some(ref s) = p.cast_session {
                                                let _ = s.set_volume(clamped);
                                            }
                                        }
                                        broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                    }
                                }
                                "start_auth" => {
                                    let restored_session = auth_ref
                                        .get_valid_session()
                                        .is_ok_and(|session| !session.access_token.is_empty());
                                    if restored_session {
                                        log::write(
                                            "Valid saved session found; restoring authentication",
                                        );
                                        ipc_ref.broadcast(
                                            &serde_json::json!({ "type": "auth_success" }),
                                        );
                                        broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                        load_favorites_async(
                                            Arc::clone(&auth_ref),
                                            Arc::clone(&player_ref),
                                            ipc_ref.clone(),
                                        );
                                    } else {
                                        if auth_flag.swap(true, Ordering::SeqCst) {
                                            println!(
                                            "  [i] Auth already in progress, requesting fresh code..."
                                        );
                                        }

                                        match auth_ref.request_device_code() {
                                            Ok(auth_info) => {
                                                log::write("Device authorization code generated");
                                                let msg = serde_json::json!({
                                                    "type": "auth_code",
                                                    "user_code": auth_info.user_code,
                                                    "verification_uri": auth_info.verification_uri,
                                                    "expires_in": auth_info.expires_in,
                                                    "interval": auth_info.interval,
                                                });
                                                ipc_ref.broadcast(&msg);

                                                // Spawn background polling worker
                                                let auth_worker = Arc::clone(&auth_ref);
                                                let ipc_worker = ipc_ref.clone();
                                                let flag_worker = Arc::clone(&auth_flag);
                                                let dev_code = auth_info.device_code.clone();
                                                let interval = auth_info.interval;
                                                let expires_in = auth_info.expires_in;
                                                let player_worker = Arc::clone(&player_ref);

                                                thread::spawn(move || {
                                                    println!(
                                                    "  [i] Polling Tidal token for device code..."
                                                );
                                                    match auth_worker
                                                        .poll_token(&dev_code, interval, expires_in)
                                                    {
                                                        Ok(session) => {
                                                            println!(
                                                            "  [✓] Successfully authenticated with Tidal! User: {:?}",
                                                            session.user_id
                                                        );
                                                            if let Err(error) =
                                                                auth_worker.save_session(&session)
                                                            {
                                                                log::write(&format!(
                                                                "Failed to save session: {error}"
                                                            ));
                                                                ipc_worker.broadcast(&serde_json::json!({
                                                                "type": "auth_error", "error": error.to_string()
                                                            }));
                                                                flag_worker
                                                                    .store(false, Ordering::SeqCst);
                                                                return;
                                                            }

                                                            let success_msg = serde_json::json!({
                                                                "type": "auth_success",
                                                                "user_id": session.user_id,
                                                            });
                                                            ipc_worker.broadcast(&success_msg);

                                                            broadcast_status(
                                                                &auth_worker,
                                                                &player_worker,
                                                                &ipc_worker,
                                                            );
                                                        }
                                                        Err(err) => {
                                                            log::write("Device authorization expired or failed");
                                                            eprintln!(
                                                            "  [!] Device auth expired or failed: {err}"
                                                        );
                                                            let expired_msg = serde_json::json!({
                                                                "type": "auth_expired",
                                                                "error": err,
                                                            });
                                                            ipc_worker.broadcast(&expired_msg);
                                                        }
                                                    }
                                                    flag_worker.store(false, Ordering::SeqCst);
                                                });
                                            }
                                            Err(e) => {
                                                log::write("Device authorization request failed");
                                                eprintln!(
                                                "  [!] Failed to request device authorization: {e}"
                                            );
                                                auth_flag.store(false, Ordering::SeqCst);
                                                let err_msg = serde_json::json!({
                                                    "type": "auth_error",
                                                    "error": e,
                                                });
                                                ipc_ref.broadcast(&err_msg);
                                            }
                                        }
                                    }
                                }
                                "logout" => {
                                    println!("  [i] User requested logout");
                                    let result = (|| {
                                        auth_ref.delete_session().map_err(|e| e.to_string())?;
                                        let mut player =
                                            player_ref.lock().map_err(|e| e.to_string())?;
                                        player.engine.stop()?;
                                        if let Some(session) = player.cast_session.take() {
                                            session.disconnect();
                                        }
                                        if let Some(ref proxy) = player.stream_proxy {
                                            proxy.clear_streams();
                                        }
                                        player.cast_active = false;
                                        player.cast_device_name = None;
                                        player.cast_state = crate::cast::CastState::Idle;
                                        player.current = None;
                                        player.favorites.clear();
                                        Ok::<(), String>(())
                                    })();
                                    if let Err(error) = result {
                                        log::write(&format!("Logout failed: {error}"));
                                        ipc_ref.broadcast(
                                            &serde_json::json!({"type":"auth_error","error":error}),
                                        );
                                    }
                                    broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                }
                                _ => {
                                    log::write("Unhandled IPC command");
                                }
                            }
                        } else {
                            log::write("Invalid IPC command JSON");
                        }
                        line.clear();
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(e) => log::write(&format!("IPC connection error: {e}")),
        }
    }
    if let Ok(mut player) = player.lock() {
        if let Some(session) = player.cast_session.take() {
            session.disconnect();
        }
        if let Err(error) = player.engine.stop() {
            log::write(&format!("Failed to stop playback: {error}"));
        }
    }
    if let Err(error) = std::fs::remove_file(ipc_server.socket_path()) {
        log::write(&format!("Failed to remove IPC socket: {error}"));
    }
    log::write("Daemon stopped");
}
