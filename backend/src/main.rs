mod api;
mod auth;
mod ipc;
mod playback;

use auth::AuthManager;
use ipc::{IpcCommand, IpcServer, IpcStateMessage};
use playback::PlaybackEngine;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

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

fn build_status_message(auth_manager: &AuthManager) -> IpcStateMessage {
    let authenticated = match auth_manager.get_valid_session() {
        Ok(s) => !s.access_token.is_empty(),
        Err(_) => false,
    };

    IpcStateMessage {
        msg_type: "status".to_string(),
        authenticated,
        is_playing: false,
        track_title: None,
        track_artist: None,
        track_album: None,
        track_art_url: None,
        duration: Some(0.0),
        position: Some(0.0),
        audio_quality: Some("LOSSLESS".to_string()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let auth_manager = Arc::new(AuthManager::new(None));

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
        println!("  --help, -h            Print this help message");
        std::process::exit(0);
    }

    println!("==> Starting Omarchy Tidal Daemon (tidal-daemon)...");
    let mut _playback = PlaybackEngine::new();
    let ipc_server = IpcServer::new();

    // Check existing session
    match auth_manager.get_valid_session() {
        Ok(session) => {
            println!(
                "  [✓] Authenticated session found for User ID: {:?}",
                session.user_id
            );
        }
        Err(_) => {
            println!("  [i] No active session found. Ready for pairing via Quickshell.");
        }
    }

    let listener = match ipc_server.bind() {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "Failed to bind IPC socket at {}: {e}",
                ipc_server.socket_path().display()
            );
            return;
        }
    };

    println!(
        "  [✓] Listening for Quickshell connections on {}",
        ipc_server.socket_path().display()
    );

    let auth_in_progress = Arc::new(AtomicBool::new(false));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let auth_ref = Arc::clone(&auth_manager);
                let ipc_ref = ipc_server.clone();
                let auth_flag = Arc::clone(&auth_in_progress);

                let write_stream = match stream.try_clone() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to clone stream: {e}");
                        continue;
                    }
                };
                ipc_ref.register_client(write_stream);

                thread::spawn(move || {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();

                    while reader.read_line(&mut line).unwrap_or(0) > 0 {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            line.clear();
                            continue;
                        }

                        if let Ok(cmd) = serde_json::from_str::<IpcCommand>(trimmed) {
                            println!("  [IPC] Command: {}", cmd.command);

                            match cmd.command.as_str() {
                                "get_status" | "get_auth_status" => {
                                    let status = build_status_message(&auth_ref);
                                    ipc_ref.broadcast(&serde_json::to_value(status).unwrap());
                                }
                                "start_auth" => {
                                    if auth_flag.swap(true, Ordering::SeqCst) {
                                        println!(
                                            "  [i] Auth already in progress, requesting fresh code..."
                                        );
                                    }

                                    match auth_ref.request_device_code() {
                                        Ok(auth_info) => {
                                            println!(
                                                "  [✓] Generated Device Code: {}",
                                                auth_info.user_code
                                            );
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

                                            thread::spawn(move || {
                                                println!(
                                                    "  [i] Polling Tidal token for device code..."
                                                );
                                                match auth_worker.poll_token(
                                                    &dev_code,
                                                    interval,
                                                    expires_in,
                                                ) {
                                                    Ok(session) => {
                                                        println!(
                                                            "  [✓] Successfully authenticated with Tidal! User: {:?}",
                                                            session.user_id
                                                        );
                                                        let _ = auth_worker.save_session(&session);

                                                        let success_msg = serde_json::json!({
                                                            "type": "auth_success",
                                                            "user_id": session.user_id,
                                                        });
                                                        ipc_worker.broadcast(&success_msg);

                                                        let status =
                                                            build_status_message(&auth_worker);
                                                        ipc_worker.broadcast(
                                                            &serde_json::to_value(status).unwrap(),
                                                        );
                                                    }
                                                    Err(err) => {
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
                                "logout" => {
                                    println!("  [i] User requested logout");
                                    let _ = auth_ref.delete_session();
                                    let status = build_status_message(&auth_ref);
                                    ipc_ref.broadcast(&serde_json::to_value(status).unwrap());
                                }
                                _ => {
                                    println!("  [i] Unhandled IPC command: {}", cmd.command);
                                }
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
