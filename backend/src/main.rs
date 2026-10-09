mod api;
mod auth;
mod ipc;
mod log;
mod mpris;
mod playback;

use api::{cover_url, Playlist, TidalApiClient, TrackItem};
use auth::AuthManager;
use ipc::{FavoriteTrack, IpcCommand, IpcServer, IpcStateMessage, PlayerMessage};
use playback::PlaybackEngine;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub(crate) static SHUTDOWN: AtomicBool = AtomicBool::new(false);

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
}

impl Player {
    fn new() -> Self {
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
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/state")
        })
        .join("omarchy/tidal/settings.json")
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
    if player.current.is_some() {
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
    }
    let track = player.current.as_ref();
    IpcStateMessage {
        msg_type: "status".to_string(),
        authenticated,
        is_playing: playing,
        track_id: track.map(|t| t.id),
        track_title: track.map(|t| t.title.clone()),
        track_artist: track.map(|t| t.artist_name().to_string()),
        track_album: track.map(|t| t.album_title().to_string()),
        track_art_url: track.map(|t| cover_url(t.cover())),
        duration: Some(track.map(|t| t.duration as f64).unwrap_or(0.0)),
        position: if playing {
            player.engine.position().ok()
        } else {
            Some(0.0)
        },
        audio_quality: Some(player.quality.clone()),
        preferred_audio_quality: Some(player.preferred_quality.clone()),
    }
}

fn broadcast_status(auth: &AuthManager, player: &Mutex<Player>, ipc: &IpcServer) {
    match player.lock() {
        Ok(player) => ipc.broadcast(&build_status_message(auth, &player)),
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

fn run_cli_probe(
    auth: &AuthManager,
    id: u64,
    quality_override: Option<&str>,
) -> Result<(), String> {
    let (api, _) = api_client(auth)?;
    let track = api.get_track(id)?;
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
    let info = api.get_playback_info(id, &requested_quality)?;
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

fn load_favorites(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
) -> Result<(), String> {
    let (api, user_id) = api_client(auth)?;
    let user_id = user_id.ok_or("Session has no user ID; please sign in again")?;
    let tracks = api.get_favorites(user_id)?;
    let mut player = player.lock().map_err(|e| e.to_string())?;
    player.favorites = tracks;
    if player.current_playlist_id.is_none() {
        player.queue = player.favorites.clone();
    }
    ipc.broadcast(&PlayerMessage::FavoritesLoaded {
        tracks: player.favorites.iter().map(FavoriteTrack::from).collect(),
    });
    Ok(())
}

fn load_playlists(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
) -> Result<(), String> {
    let (api, user_id) = api_client(auth)?;
    let user_id = user_id.ok_or("Session has no user ID; please sign in again")?;
    let playlists = api.get_playlists(user_id)?;
    player.lock().map_err(|e| e.to_string())?.playlists = playlists.clone();
    ipc.broadcast(&PlayerMessage::PlaylistsLoaded { playlists });
    Ok(())
}

fn load_playlist_tracks(
    auth: &AuthManager,
    player: &Mutex<Player>,
    ipc: &IpcServer,
    playlist_id: String,
) -> Result<(), String> {
    let (api, _) = api_client(auth)?;
    let tracks = api.get_playlist_tracks(&playlist_id)?;
    let mut state = player.lock().map_err(|e| e.to_string())?;
    state.queue = tracks.clone();
    state.current_playlist_id = Some(playlist_id.clone());
    ipc.broadcast(&PlayerMessage::PlaylistTracksLoaded {
        playlist_id: &playlist_id,
        tracks: tracks.iter().map(FavoriteTrack::from).collect(),
    });
    Ok(())
}

fn search_catalog(
    auth: &AuthManager,
    ipc: &IpcServer,
    query: Option<String>,
) -> Result<(), String> {
    let query = query.unwrap_or_default();
    if query.trim().is_empty() {
        ipc.broadcast(&PlayerMessage::SearchResults {
            results: Vec::new(),
        });
        return Ok(());
    }
    let (api, _) = api_client(auth)?;
    let tracks = api.search(query.trim())?;
    ipc.broadcast(&PlayerMessage::SearchResults {
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
    let (api, _) = api_client(auth)?;
    let mut player = player.lock().map_err(|e| e.to_string())?;
    let track = match player
        .queue
        .iter()
        .chain(player.favorites.iter())
        .find(|track| track.id == id)
    {
        Some(track) => track.clone(),
        None => api.get_track(id)?,
    };
    if !player.queue.iter().any(|item| item.id == id) {
        if player.favorites.iter().any(|item| item.id == id) {
            player.queue = player.favorites.clone();
            player.current_playlist_id = None;
        } else {
            player.queue = vec![track.clone()];
            player.current_playlist_id = None;
        }
    }
    let preferred_quality = player.preferred_quality.clone();
    log::write(&format!(
        "Playback track: id={id}; title={}; artist={}; catalog audio quality={:?}",
        track.title,
        track.artist_name(),
        track.audio_quality
    ));
    let info = api.get_playback_info(id, &preferred_quality)?;
    let mime = match info.manifest_mime_type.as_str() {
        "application/vnd.tidal.bts" | "application/dash+xml" => info.manifest_mime_type.as_str(),
        _ => "unsupported",
    };
    log::write(&format!("Playback manifest MIME: {mime}"));
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
        load_favorites(auth, player, ipc)?;
        let first = {
            let state = player.lock().map_err(|e| e.to_string())?;
            state
                .queue
                .first()
                .or_else(|| state.favorites.first())
                .map(|track| track.id)
        };
        if let Some(id) = first {
            return play_track(auth, player, ipc, Some(id));
        }
        return Err("No favorite tracks are available".to_string());
    }
    let state = player.lock().map_err(|e| e.to_string())?;
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
        if !forward && state.engine.position().unwrap_or(0.0) > 3.0 {
            state.engine.seek(0.0)?;
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
        if forward {
            (index + 1) % len
        } else {
            (index + len - 1) % len
        }
    };
    let id = player.lock().map_err(|e| e.to_string())?.queue[target].id;
    play_track(auth, player, ipc, Some(id))
}

fn start_position_ticker(auth: Arc<AuthManager>, player: Arc<Mutex<Player>>, ipc: IpcServer) {
    thread::spawn(move || {
        while !SHUTDOWN.load(Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_millis(250));
            let (position, duration, is_playing, eof_track) = match player.lock() {
                Ok(mut state) => {
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
                            (None, None, false, Some(id))
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
                            )
                        }
                    } else {
                        (None, None, false, None)
                    }
                }
                Err(_) => continue,
            };
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

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Omarchy Tidal Music Player Daemon (tidal-daemon)");
        println!("Usage: tidal-daemon [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --test-auth, --login  Run OAuth 2.0 Device Flow authorization CLI runner");
        println!("  --status              Print current session and authentication status");
        println!("  --logout              Clear stored session credentials");
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
                                | "get_playlist_tracks"
                                | "set_audio_quality"
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
                                    if let Err(error) =
                                        load_favorites(&auth_ref, &player_ref, &ipc_ref)
                                    {
                                        log::write(&format!("Favorites failed: {error}"));
                                        ipc_ref.broadcast(&PlayerMessage::FavoritesError {
                                            error: &error,
                                        });
                                    }
                                }
                                "search" => {
                                    if let Err(error) =
                                        search_catalog(&auth_ref, &ipc_ref, cmd.query)
                                    {
                                        log::write(&format!("Search failed: {error}"));
                                        ipc_ref.broadcast(&PlayerMessage::SearchError {
                                            error: &error,
                                        });
                                    }
                                }
                                "get_playlists" => {
                                    if let Err(error) =
                                        load_playlists(&auth_ref, &player_ref, &ipc_ref)
                                    {
                                        ipc_ref.broadcast(&PlayerMessage::PlaylistsError {
                                            error: &error,
                                        });
                                    }
                                }
                                "get_playlist_tracks" => {
                                    let playlist_id =
                                        cmd.playlist_id.or(cmd.uuid).unwrap_or_default();
                                    if playlist_id.is_empty() {
                                        ipc_ref.broadcast(&PlayerMessage::PlaylistTracksError {
                                            playlist_id: "",
                                            error: "playlist_id is required",
                                        });
                                    } else if let Err(error) = load_playlist_tracks(
                                        &auth_ref,
                                        &player_ref,
                                        &ipc_ref,
                                        playlist_id.clone(),
                                    ) {
                                        ipc_ref.broadcast(&PlayerMessage::PlaylistTracksError {
                                            playlist_id: &playlist_id,
                                            error: &error,
                                        });
                                    }
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
                                        broadcast_status(&auth_ref, &player_ref, &ipc_ref);
                                        Ok::<(), String>(())
                                    })();
                                    if let Err(error) = result {
                                        ipc_ref.broadcast(&serde_json::json!({"type":"playback_error","error":error}));
                                    }
                                }
                                "play_track" => {
                                    if let Err(error) =
                                        play_track(&auth_ref, &player_ref, &ipc_ref, cmd.track_id)
                                    {
                                        log::write(&format!("Playback failed: {error}"));
                                        ipc_ref.broadcast(&PlayerMessage::PlaybackError {
                                            error: &error,
                                        });
                                    }
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
                                        if let Err(error) =
                                            load_favorites(&auth_ref, &player_ref, &ipc_ref)
                                        {
                                            log::write(&format!("Favorites failed during session restoration: {error}"));
                                            ipc_ref.broadcast(&PlayerMessage::FavoritesError {
                                                error: &error,
                                            });
                                        }
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
        if let Err(error) = player.engine.stop() {
            log::write(&format!("Failed to stop playback: {error}"));
        }
    }
    if let Err(error) = std::fs::remove_file(ipc_server.socket_path()) {
        log::write(&format!("Failed to remove IPC socket: {error}"));
    }
    log::write("Daemon stopped");
}
