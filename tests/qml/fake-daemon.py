#!/usr/bin/env python3
import json
import os
import socket

path = os.path.join(os.environ["XDG_RUNTIME_DIR"], "tidal.sock")
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
    server.bind(path)
    server.listen(1)
    connection, _ = server.accept()
    authenticated = False
    playing = False
    track = {
        "id": 42, "title": "Test song", "artist": "Test artist",
        "album": "Test album", "duration": 180, "cover": "",
        "art_url": "file://" + os.path.join(os.path.dirname(__file__), "cover.svg"),
    }
    with connection, connection.makefile("r") as reader:
        for line in reader:
            message = json.loads(line)
            command = message["command"]
            if command == "start_auth":
                authenticated = True
                connection.sendall(b'{"type":"auth_success"}\n')
            elif command == "get_status":
                status = {
                    "type": "status", "authenticated": authenticated, "is_playing": playing,
                    "track_id": track["id"] if playing else None,
                    "track_title": track["title"] if playing else "",
                    "track_artist": track["artist"] if playing else "",
                    "track_album": track["album"] if playing else "",
                    "track_art_url": track["art_url"] if playing else "",
                    "duration": track["duration"] if playing else 0,
                }
                connection.sendall((json.dumps(status) + "\n").encode())
            elif command == "get_favorites":
                connection.sendall((json.dumps({"type": "favorites_loaded", "tracks": [track]}) + "\n").encode())
            elif command == "play_track":
                if message["track_id"] != track["id"]:
                    raise ValueError("Unexpected track_id")
                playing = True
                connection.sendall((json.dumps({"type": "playback_started", "track_id": track["id"]}) + "\n").encode())
                connection.sendall((json.dumps({
                    "type": "status", "authenticated": True, "is_playing": True,
                    "track_id": track["id"],
                    "track_title": track["title"], "track_artist": track["artist"],
                    "track_album": track["album"], "track_art_url": track["art_url"],
                    "duration": track["duration"],
                }) + "\n").encode())
