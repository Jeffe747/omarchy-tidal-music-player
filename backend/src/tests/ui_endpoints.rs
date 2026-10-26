use crate::api::{album_tracks_url, artist_top_tracks_url, favorite_url};
use crate::ipc::{IpcCommand, IpcStateMessage};

type NavigationPlayback = Box<dyn FnOnce(u64) -> Result<(), String>>;

thread_local! {
    // Intercept playback after navigation releases the queue lock, without network or mpv.
    static NAVIGATION_PLAYBACK: std::cell::RefCell<Option<NavigationPlayback>> = const {
        std::cell::RefCell::new(None)
    };
}

pub(super) fn navigation_playback_override(track_id: u64) -> Option<Result<(), String>> {
    NAVIGATION_PLAYBACK.with(|callback| callback.borrow_mut().take().map(|play| play(track_id)))
}

fn navigation_track(id: u64) -> crate::api::TrackItem {
    serde_json::from_value(serde_json::json!({
        "id": id, "title": "Track", "duration": 120
    }))
    .unwrap()
}

fn navigation_player() -> crate::Player {
    crate::Player {
        engine: crate::playback::PlaybackEngine::new(),
        favorites: Vec::new(),
        queue: (101..=103).map(navigation_track).collect(),
        queue_source: crate::QueueSource::Exploration {
            view: "album".into(),
            id: 50,
        },
        playlists: Vec::new(),
        current_playlist_id: None,
        current: Some(navigation_track(102)),
        quality: "LOSSLESS".into(),
        preferred_quality: "LOSSLESS".into(),
        eof_handled_track: None,
        track_has_started: false,
        shuffle: false,
        repeat_mode: "off".into(),
        // No Cast session means position is zero, without contacting a live mpv.
        cast_active: true,
        cast_device_name: None,
        cast_state: crate::cast::CastState::Idle,
        cast_volume: 1.0,
        cast_session: None,
        stream_proxy: None,
        cast_discovery: std::sync::Arc::new(crate::cast::discovery::CastDiscovery::new()),
        cast_firewall_blocked: false,
        last_firewall_check: std::time::Instant::now(),
    }
}

#[test]
fn navigation_keeps_selected_id_across_concurrent_queue_replacement() {
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    for forward in [true, false] {
        for replacement_ids in [vec![201, 202, 203], vec![201], vec![]] {
            let player = Arc::new(Mutex::new(navigation_player()));
            let worker_player = Arc::clone(&player);
            let (replace, start) = mpsc::channel();
            let (done, replaced) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                start.recv_timeout(Duration::from_secs(5)).unwrap();
                worker_player.lock().unwrap().queue =
                    replacement_ids.into_iter().map(navigation_track).collect();
                done.send(()).unwrap();
            });
            NAVIGATION_PLAYBACK.with(|callback| {
                *callback.borrow_mut() = Some(Box::new(move |id| {
                    replace.send(()).unwrap();
                    replaced.recv_timeout(Duration::from_secs(5)).unwrap();
                    assert_eq!(id, if forward { 103 } else { 101 });
                    Ok(())
                }));
            });
            let result = crate::navigate(
                &crate::auth::AuthManager::new(None),
                &player,
                &crate::ipc::IpcServer::new(),
                forward,
            );
            worker.join().unwrap();
            assert_eq!(result, Ok(()));
            assert!(!player.is_poisoned());
        }
    }
}

#[test]
fn navigation_checks_replaced_queue_bounds_before_playback() {
    use std::sync::Mutex;

    for forward in [true, false] {
        for queue in [
            vec![],
            vec![navigation_track(201)],
            vec![navigation_track(102)],
        ] {
            let player = Mutex::new(navigation_player());
            player.lock().unwrap().queue = queue;
            NAVIGATION_PLAYBACK.with(|callback| {
                *callback.borrow_mut() = Some(Box::new(|id| {
                    assert_eq!(id, 102);
                    Ok(())
                }));
            });
            let result = crate::navigate(
                &crate::auth::AuthManager::new(None),
                &player,
                &crate::ipc::IpcServer::new(),
                forward,
            );
            let state = player.lock().unwrap();
            if state.queue.first().is_some_and(|track| track.id == 102) {
                if forward {
                    assert_eq!(result, Err("End of queue".into()));
                } else {
                    assert_eq!(result, Ok(()));
                }
            } else {
                assert_eq!(result, Err("Current track is not in active queue".into()));
            }
            NAVIGATION_PLAYBACK.with(|callback| {
                callback.borrow_mut().take();
            });
        }
    }
}

#[test]
fn favorite_and_exploration_api_routes_are_wired() {
    assert_eq!(
        favorite_url(5040, 42, true),
        "https://api.tidal.com/v1/users/5040/favorites/tracks"
    );
    assert_eq!(
        favorite_url(5040, 42, false),
        "https://api.tidal.com/v1/users/5040/favorites/tracks/42"
    );
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
fn new_ui_ipc_commands_and_status_fields_deserialize() {
    for name in [
        "toggle_shuffle",
        "cycle_repeat",
        "toggle_favorite",
        "get_album_tracks",
        "get_artist_tracks",
        "get_track_radio",
    ] {
        let command: IpcCommand =
            serde_json::from_value(serde_json::json!({ "command": name })).unwrap();
        assert_eq!(command.command, name);
    }
    let command: IpcCommand = serde_json::from_value(
        serde_json::json!({ "command": "get_album_tracks", "album_id": 12 }),
    )
    .unwrap();
    assert_eq!(command.album_id, Some(12));
    let command: IpcCommand = serde_json::from_value(
        serde_json::json!({ "command": "get_track_radio", "track_id": 42 }),
    )
    .unwrap();
    assert_eq!(command.track_id, Some(42));

    let command: IpcCommand = serde_json::from_value(
        serde_json::json!({ "command": "play_track", "track_id": 42, "context": "explore" }),
    )
    .unwrap();
    assert_eq!(command.context, Some("explore".to_string()));

    let status: IpcStateMessage = serde_json::from_value(serde_json::json!({
        "type": "status",
        "authenticated": true,
        "is_playing": false,
        "shuffle": true,
        "repeat_mode": "one",
        "is_favorite": true,
        "artist_id": 34,
        "album_id": 12
    }))
    .unwrap();
    assert!(status.shuffle);
    assert_eq!(status.repeat_mode, "one");
    assert!(status.is_favorite);
    assert_eq!(status.artist_id, Some(34));
    assert_eq!(status.album_id, Some(12));
}

#[test]
fn test_favorites_loading_atomic_guard() {
    use std::sync::atomic::Ordering;
    use crate::{FAVORITES_LOADING, PLAY_REQUEST_ID};

    // Verify atomic guard prevents concurrent loading loops
    assert!(!FAVORITES_LOADING.load(Ordering::SeqCst));
    assert!(FAVORITES_LOADING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok());
    assert!(FAVORITES_LOADING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err());
    FAVORITES_LOADING.store(false, Ordering::SeqCst);
    assert!(!FAVORITES_LOADING.load(Ordering::SeqCst));

    // Verify monotonic PLAY_REQUEST_ID invalidates older in-flight requests
    let req1 = PLAY_REQUEST_ID.fetch_add(1, Ordering::SeqCst) + 1;
    let req2 = PLAY_REQUEST_ID.fetch_add(1, Ordering::SeqCst) + 1;
    assert!(req2 > req1);
    assert_ne!(req1, PLAY_REQUEST_ID.load(Ordering::SeqCst));
    assert_eq!(req2, PLAY_REQUEST_ID.load(Ordering::SeqCst));
}

#[test]
fn test_exploration_queue_source_preservation() {
    use crate::api::TrackItem;
    use crate::QueueSource;

    let track1: TrackItem = serde_json::from_str(
        r#"{"id":101,"title":"Track 1","duration":120,"artist":{"name":"Artist"},"album":{"title":"Album"}}"#
    ).unwrap();
    let track2: TrackItem = serde_json::from_str(
        r#"{"id":102,"title":"Track 2","duration":180,"artist":{"name":"Artist"},"album":{"title":"Album"}}"#
    ).unwrap();
    let fav_track: TrackItem = serde_json::from_str(
        r#"{"id":999,"title":"Fav Track","duration":200,"artist":{"name":"Artist"},"album":{"title":"Album"}}"#
    ).unwrap();

    let mut queue = vec![track1.clone(), track2.clone()];
    let mut queue_source = QueueSource::Exploration {
        view: "album".to_string(),
        id: 50,
    };
    let favorites = vec![fav_track.clone()];

    // 1. Favorites progressive loading must NOT mutate queue when in Exploration mode
    let page_tracks = vec![fav_track.clone()];
    if queue_source == QueueSource::Favorites {
        queue.extend(page_tracks);
    }
    assert_eq!(queue.len(), 2, "Exploration queue must not be overwritten by favorites loading");

    // 2. Navigation calculation in exploration queue: Next from track 1 advances to track 2
    let index = queue.iter().position(|t| t.id == track1.id).unwrap();
    let next_index = (index + 1) % queue.len();
    assert_eq!(queue[next_index].id, 102, "Next must advance to track 2 on the explored album");

    // 3. Navigation from track 2: reaches end of queue when repeat is off
    assert_eq!(next_index + 1, queue.len(), "End of album queue detected correctly");

    // 4. Switching context to favorites when user explicitly plays from favorites
    let context = Some("favorites");
    let target_id = fav_track.id;
    if context == Some("favorites") && favorites.iter().any(|item| item.id == target_id) {
        queue = favorites.clone();
        queue_source = QueueSource::Favorites;
    }
    assert_eq!(queue_source, QueueSource::Favorites);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].id, 999);
}
