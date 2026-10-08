#!/usr/bin/env python3
import json
import os
import socket
import sys
import time

path = next(arg.split("=", 1)[1] for arg in sys.argv if arg.startswith("--input-ipc-server="))
with open(path + ".args", "w") as output:
    json.dump(sys.argv[1:], output)
time.sleep(0.2)
paused = True
idle = True
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
    server.bind(path)
    server.listen()
    while True:
        connection, _ = server.accept()
        with connection, connection.makefile("r") as reader:
            line = reader.readline()
            if not line:
                continue
            message = json.loads(line)
            command = message["command"]
            with open(path + ".commands", "a") as output:
                output.write(json.dumps(command) + "\n")
            response = {"request_id": message["request_id"], "error": "success"}
            if command[0] == "loadfile":
                if command[1] == "fail":
                    response["error"] = "loading failed"
                else:
                    idle = False
            elif command[:2] == ["set_property", "pause"]:
                paused = command[2]
            elif command[:2] == ["cycle", "pause"]:
                paused = not paused
            elif command[0] == "get_property":
                response["data"] = {"pause": paused, "idle-active": idle}[command[1]]
            connection.sendall(b'{"event":"idle"}\n')
            connection.sendall((json.dumps(response) + "\n").encode())
