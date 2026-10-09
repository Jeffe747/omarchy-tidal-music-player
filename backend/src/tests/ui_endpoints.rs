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
