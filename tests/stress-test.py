#!/usr/bin/env python3
"""Isolated IPC stress test for tidal-daemon (no real account/session required)."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "bin/tidal-daemon"
TIMEOUT = 3.0


def proc_metrics(pid):
    status = Path(f"/proc/{pid}/status").read_text()
    rss_kb = int(next(line.split()[1] for line in status.splitlines()
                      if line.startswith("VmRSS:")))
    return rss_kb, len(list(Path(f"/proc/{pid}/fd").iterdir()))


def connect(path, timeout=TIMEOUT):
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(timeout)
    client.connect(str(path))
    return client


def send_request(path, message, timeout=TIMEOUT):
    with connect(path, timeout) as client:
        client.sendall((json.dumps(message, allow_nan=True) + "\n").encode())
        data = b""
        while b"\n" not in data:
            chunk = client.recv(65536)
            if not chunk:
                break
            data += chunk
        return json.loads(data.split(b"\n", 1)[0]) if data else None


class Client:
    def __init__(self, path):
        self.sock = connect(path, 15)
        self.buffer = b""

    def request(self, message):
        self.sock.sendall((json.dumps(message, allow_nan=True) + "\n").encode())
        while b"\n" not in self.buffer:
            chunk = self.sock.recv(65536)
            if not chunk:
                return None
            self.buffer += chunk
        line, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(line)

    def close(self):
        self.sock.close()


def wait_socket(path, daemon):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if daemon.poll() is not None:
            raise RuntimeError(f"daemon exited during startup ({daemon.returncode})")
        if path.exists():
            try:
                with connect(path):
                    return
            except (OSError, TimeoutError):
                pass
        time.sleep(0.02)
    raise RuntimeError("daemon socket did not become available")


def main():
    if not BINARY.is_file() or not os.access(BINARY, os.X_OK):
        raise SystemExit(f"Missing executable bundle: {BINARY}; run ./scripts/build.sh")

    counters = {"requests": 0, "crashes": 0, "timeouts": 0, "malformed": 0}
    start = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="tidal-stress-") as runtime:
        runtime_path = Path(runtime)
        env = dict(os.environ, HOME=runtime, XDG_RUNTIME_DIR=runtime,
                   XDG_STATE_HOME=f"{runtime}/state",
                   TIDAL_SESSION_PATH=f"{runtime}/session.json")
        daemon = subprocess.Popen([str(BINARY)], env=env, stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL)
        path = runtime_path / "tidal.sock"
        try:
            wait_socket(path, daemon)
            # 1,400 sequential interleaved transport calls across persistent clients.
            commands = ["play", "pause", "toggle_play", "next", "previous",
                        "toggle_shuffle", "cycle_repeat"]
            client = connect(path)
            client.settimeout(TIMEOUT)
            try:
                for i in range(1400):
                    client.sendall((json.dumps({"command": commands[i % len(commands)]}) + "\n").encode())
                    # Drain one broadcast event, keeping the socket bounded.
                    if client.recv(65536):
                        pass
                    counters["requests"] += 1
                    if daemon.poll() is not None:
                        counters["crashes"] += 1
                        raise RuntimeError("daemon crashed in transport flood")
            finally:
                client.close()

            # Invalid and extreme numeric inputs plus rapid scrub values.
            seeks = [-10, 1e9, 0, float("nan"), float("inf"), -float("inf"),
                     1.7976931348623157e308, -1.7976931348623157e308]
            scrub = Client(path)
            try:
                for i in range(400):
                    value = seeks[i % len(seeks)] if i < len(seeks) else ((i % 101) / 10)
                    scrub.request({"command": "seek", "position": value})
                    counters["requests"] += 1
            finally:
                scrub.close()

            # Truncation, garbage, oversized lines, invalid UTF-8 and injection-like strings.
            malformed_payloads = [b'{"command":"get_status"', b"\xff\xfe\xfa\n",
                                  b"{" + b"x" * 65536 + b"\n",
                                  b'{"command":"get_status","query":"\' OR 1=1; DROP TABLE users;--"}\n',
                                  b"not-json\n"]
            for payload in malformed_payloads:
                with connect(path) as sock:
                    sock.sendall(payload)
                counters["malformed"] += 1
            # Confirm parser and daemon remain responsive after fuzz inputs.
            assert Client(path).request({"command": "get_status"})["type"] == "status"
            counters["requests"] += 1

            # 50 overlapping clients, each issuing a command then closing abruptly.
            barrier = threading.Barrier(51)
            errors = []
            def concurrent_client(i):
                try:
                    with connect(path) as sock:
                        barrier.wait(timeout=5)
                        sock.sendall((json.dumps({"command": "get_status" if i % 2 else "toggle_shuffle"}) + "\n").encode())
                        # Close without reading the reply, like a crashed widget client.
                    counters["requests"] += 1
                except Exception as error:
                    errors.append(repr(error))
            workers = [threading.Thread(target=concurrent_client, args=(i,)) for i in range(50)]
            for worker in workers:
                worker.start()
            barrier.wait(timeout=8)
            for worker in workers:
                worker.join(timeout=8)
            if errors or any(worker.is_alive() for worker in workers):
                raise RuntimeError(f"concurrent client failures/hangs: {errors}")

            # 2,000 additional completed status transactions, sampling resource use.
            rss_before, fd_before = proc_metrics(daemon.pid)
            resource_client = Client(path)
            try:
                for _ in range(2000):
                    response = resource_client.request({"command": "get_status"})
                    if not response or response.get("type") != "status":
                        raise RuntimeError(f"unexpected stress response: {response!r}")
                    counters["requests"] += 1
                    if daemon.poll() is not None:
                        counters["crashes"] += 1
                        raise RuntimeError("daemon crashed during resource leak phase")
            finally:
                resource_client.close()
            time.sleep(0.25)
            rss_after, fd_after = proc_metrics(daemon.pid)

            # No session means the isolated daemon cannot have active mpv playback.
            mpv_recovery = "SKIPPED (isolated no-session run; live recovery covered by playback unit test)"
            if Path(runtime, "session.json").exists():
                mpv_recovery = "not exercised"

            elapsed = time.monotonic() - start
            print("Tidal daemon IPC stress results")
            print(f"Requests sent: {counters['requests']} (malformed payloads: {counters['malformed']})")
            print(f"Elapsed: {elapsed:.2f}s; throughput: {counters['requests'] / elapsed:.1f} requests/s")
            print(f"RSS: {rss_before} KiB -> {rss_after} KiB ({rss_after-rss_before:+d} KiB)")
            print(f"FDs: {fd_before} -> {fd_after} ({fd_after-fd_before:+d})")
            print(f"Timeouts: {counters['timeouts']}; daemon crashes: {counters['crashes']}")
            print(f"Active mpv kill/recovery: {mpv_recovery}")
            if abs(rss_after - rss_before) > 16384 or fd_after - fd_before > 2:
                raise RuntimeError("resource growth exceeded threshold (16 MiB RSS or 2 FDs)")
        finally:
            daemon.terminate()
            try:
                daemon.wait(timeout=8)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.wait()
                raise RuntimeError("daemon did not stop cleanly")
            if daemon.returncode != 0:
                raise RuntimeError(f"daemon exit status {daemon.returncode}")
            if path.exists():
                raise RuntimeError("daemon left its IPC socket behind")


if __name__ == "__main__":
    main()
