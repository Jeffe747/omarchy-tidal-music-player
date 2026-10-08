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
    with connection, connection.makefile("r") as reader:
        for line in reader:
            command = json.loads(line)["command"]
            if command == "start_auth":
                authenticated = True
                connection.sendall(b'{"type":"auth_success"}\n')
            elif command == "get_status":
                status = {"type": "status", "authenticated": authenticated}
                connection.sendall((json.dumps(status) + "\n").encode())
