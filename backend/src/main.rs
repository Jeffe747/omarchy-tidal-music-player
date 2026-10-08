mod api;
mod auth;
mod ipc;
mod playback;

use auth::AuthManager;
use ipc::{IpcCommand, IpcServer, IpcStateMessage};
use playback::PlaybackEngine;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    println!("==> Starting Omarchy Tidal Daemon (tidal-daemon)...");

    let auth_manager = Arc::new(AuthManager::new(None));
    let mut playback = PlaybackEngine::new();
    let ipc_server = IpcServer::new();

    let session = auth_manager.load_session();
    let is_authenticated = session.is_some();
    println!("  [i] Authenticated: {}", is_authenticated);

    let listener = match ipc_server.bind() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind IPC socket: {e}");
            return;
        }
    };

    println!("  [✓] Listening for Quickshell connections on tidal.sock");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let auth_ref = Arc::clone(&auth_manager);
                thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();

                    while reader.read_line(&mut line).unwrap_or(0) > 0 {
                        if let Ok(cmd) = serde_json::from_str::<IpcCommand>(&line) {
                            println!("Received IPC command: {}", cmd.command);

                            match cmd.command.as_str() {
                                "get_status" => {
                                    let status = IpcStateMessage {
                                        msg_type: "status".to_string(),
                                        authenticated: auth_ref.load_session().is_some(),
                                        is_playing: false,
                                        track_title: None,
                                        track_artist: None,
                                        track_album: None,
                                        track_art_url: None,
                                        duration: Some(0.0),
                                        position: Some(0.0),
                                        audio_quality: Some("LOSSLESS".to_string()),
                                    };
                                    let _ = IpcServer::send_message(
                                        &mut stream,
                                        &serde_json::to_value(status).unwrap(),
                                    );
                                }
                                "start_auth" => {
                                    if let Ok(auth_info) = auth_ref.request_device_code() {
                                        let msg = serde_json::json!({
                                            "type": "auth_code",
                                            "user_code": auth_info.user_code,
                                            "verification_uri": auth_info.verification_uri,
                                        });
                                        let _ = IpcServer::send_message(&mut stream, &msg);

                                        // Poll in background
                                        let auth_worker = Arc::clone(&auth_ref);
                                        let dev_code = auth_info.device_code.clone();
                                        thread::spawn(move || {
                                            if let Ok(session) = auth_worker.poll_token(&dev_code) {
                                                let _ = auth_worker.save_session(&session);
                                                println!("Successfully authenticated with Tidal!");
                                            }
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                        line.clear();
                    }
                });
            }
            Err(e) => eprintln!("IPC connection error: {e}"),
        }
    }
}
