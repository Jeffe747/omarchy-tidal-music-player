#!/usr/bin/env python3
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

binary = Path(__file__).resolve().parents[1] / "bin/tidal-daemon"
with tempfile.TemporaryDirectory(prefix="tidal-daemon-smoke-") as runtime:
    env = dict(os.environ, XDG_RUNTIME_DIR=runtime, TIDAL_SESSION_PATH=runtime + "/session.json")
    daemon = subprocess.Popen([binary], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        path = runtime + "/tidal.sock"
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(5)
            deadline = time.monotonic() + 5
            while True:
                try:
                    client.connect(path)
                    break
                except (FileNotFoundError, ConnectionRefusedError):
                    if daemon.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("Daemon did not become responsive")
                    time.sleep(0.025)
            with client.makefile("r") as reader:
                def request(message):
                    client.sendall((json.dumps(message) + "\n").encode())
                    return json.loads(reader.readline())

                status = request({"command": "get_status"})
                assert status["type"] == "status"
                assert status["authenticated"] is False and status["is_playing"] is False
                assert status["track_title"] is None and status["duration"] == 0
                assert request({"command": "get_favorites"}) == {
                    "type": "favorites_error", "error": "No active session found",
                }
                assert request({"command": "play_track", "track_id": 42}) == {
                    "type": "playback_error", "error": "No active session found",
                }
                assert request({"command": "play_track"}) == {
                    "type": "playback_error", "error": "play_track requires track_id",
                }
                assert request({"command": "get_status"})["type"] == "status"
    finally:
        daemon.terminate()
        try:
            stdout, stderr = daemon.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            daemon.kill()
            stdout, stderr = daemon.communicate()
            raise RuntimeError("Daemon failed to shut down")
        if daemon.returncode != 0:
            raise RuntimeError((stdout + stderr).decode())
    assert not Path(runtime, "tidal.sock").exists(), "Daemon must clean up its socket"
    assert not Path(runtime, "tidal-mpv.sock").exists()
    print("Daemon smoke passed: status, favorites/playback errors, responsive IPC, clean shutdown")
