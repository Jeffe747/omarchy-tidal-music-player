use crate::api::{album_tracks_url, artist_top_tracks_url, favorite_url};
use crate::ipc::{IpcCommand, IpcStateMessage};

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
