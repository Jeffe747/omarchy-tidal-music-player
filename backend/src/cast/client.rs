use super::{CastDevice, CastMessage, CastState};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

#[derive(Debug)]
struct NoCertVerifier;

impl ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

pub fn make_tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("TLS safe protocol configuration error: {e}"))?
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
    .with_no_client_auth();
    Ok(Arc::new(config))
}

pub enum CastAction {
    Load {
        stream_url: String,
        mime_type: String,
        title: String,
        artist: String,
        album: String,
        art_url: String,
        position: f64,
    },
    Play,
    Pause,
    Seek(f64),
    Stop,
    SetVolume(f64),
    Disconnect,
}

#[derive(Clone)]
pub struct CastSession {
    pub device: CastDevice,
    command_tx: Sender<CastAction>,
    state: Arc<Mutex<CastState>>,
    position: Arc<Mutex<f64>>,
    last_position_update: Arc<Mutex<Instant>>,
    is_active: Arc<AtomicBool>,
}

impl CastSession {
    pub fn connect(
        device: CastDevice,
        on_track_finished: Arc<dyn Fn() + Send + Sync + 'static>,
    ) -> Result<Self, String> {
        let (tx, rx) = channel();
        let state = Arc::new(Mutex::new(CastState::Connecting));
        let position = Arc::new(Mutex::new(0.0));
        let last_position_update = Arc::new(Mutex::new(Instant::now()));
        let is_active = Arc::new(AtomicBool::new(true));

        let state_clone = Arc::clone(&state);
        let pos_clone = Arc::clone(&position);
        let pos_update_clone = Arc::clone(&last_position_update);
        let active_clone = Arc::clone(&is_active);
        let dev_clone = device.clone();

        thread::spawn(move || {
            run_session_worker(
                dev_clone,
                rx,
                state_clone,
                pos_clone,
                pos_update_clone,
                active_clone,
                on_track_finished,
            );
        });

        // Wait briefly for connection establishment (up to 3 seconds)
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            if let Ok(st) = state.lock() {
                if *st != CastState::Connecting {
                    break;
                }
            }
            if !is_active.load(Ordering::Relaxed) {
                return Err(format!("Failed to connect to Cast device {}", device.name));
            }
            thread::sleep(Duration::from_millis(50));
        }

        Ok(Self {
            device,
            command_tx: tx,
            state,
            position,
            last_position_update,
            is_active,
        })
    }

    pub fn state(&self) -> CastState {
        self.state.lock().map(|s| *s).unwrap_or(CastState::Idle)
    }

    pub fn position(&self) -> f64 {
        let base = self.position.lock().map(|p| *p).unwrap_or(0.0);
        let state = self.state();
        if state == CastState::Playing {
            let elapsed = self
                .last_position_update
                .lock()
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.0);
            base + elapsed
        } else {
            base
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    pub fn load_track(
        &self,
        stream_url: String,
        mime_type: String,
        title: String,
        artist: String,
        album: String,
        art_url: String,
        position: f64,
    ) -> Result<(), String> {
        self.command_tx
            .send(CastAction::Load {
                stream_url,
                mime_type,
                title,
                artist,
                album,
                art_url,
                position,
            })
            .map_err(|e| format!("Failed to send load command: {e}"))
    }

    pub fn play(&self) -> Result<(), String> {
        self.command_tx
            .send(CastAction::Play)
            .map_err(|e| format!("Failed to send play command: {e}"))
    }

    pub fn pause(&self) -> Result<(), String> {
        self.command_tx
            .send(CastAction::Pause)
            .map_err(|e| format!("Failed to send pause command: {e}"))
    }

    pub fn seek(&self, seconds: f64) -> Result<(), String> {
        self.command_tx
            .send(CastAction::Seek(seconds))
            .map_err(|e| format!("Failed to send seek command: {e}"))
    }

    pub fn stop(&self) -> Result<(), String> {
        self.command_tx
            .send(CastAction::Stop)
            .map_err(|e| format!("Failed to send stop command: {e}"))
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.command_tx
            .send(CastAction::SetVolume(volume))
            .map_err(|e| format!("Failed to send set_volume command: {e}"))
    }

    pub fn disconnect(&self) {
        let _ = self.command_tx.send(CastAction::Disconnect);
        self.is_active.store(false, Ordering::Relaxed);
        if let Ok(mut st) = self.state.lock() {
            *st = CastState::Idle;
        }
    }
}

fn write_cast_message<W: Write>(
    writer: &mut W,
    msg: &CastMessage,
) -> Result<(), std::io::Error> {
    let payload = msg.to_bytes();
    let len = (payload.len() as u32).to_be_bytes();
    writer.write_all(&len)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

fn read_cast_message<R: Read>(reader: &mut R) -> Result<CastMessage, std::io::Error> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 65536 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Cast message length exceeds 64KB",
        ));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    CastMessage::from_bytes(&payload)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn run_session_worker(
    device: CastDevice,
    rx: Receiver<CastAction>,
    state: Arc<Mutex<CastState>>,
    position: Arc<Mutex<f64>>,
    last_position_update: Arc<Mutex<Instant>>,
    is_active: Arc<AtomicBool>,
    on_track_finished: Arc<dyn Fn() + Send + Sync + 'static>,
) {
    let target = format!("{}:{}", device.ip, device.port);
    crate::log::write(&format!("Connecting to Cast device at {target}"));

    let addr = match target.parse::<SocketAddr>() {
        Ok(a) => a,
        Err(e) => {
            crate::log::write(&format!("Invalid Cast device target address {target}: {e}"));
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };

    let tcp_stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
        Ok(s) => s,
        Err(e) => {
            crate::log::write(&format!("TCP connect error to {target}: {e}"));
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };

    let _ = tcp_stream.set_nodelay(true);
    let _ = tcp_stream.set_read_timeout(Some(Duration::from_millis(100)));

    let tls_config = match make_tls_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            crate::log::write(&format!("TLS config error: {e}"));
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };
    let server_name = match ServerName::try_from("chromecast".to_string()) {
        Ok(s) => s,
        Err(_) => {
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };

    let conn = match rustls::ClientConnection::new(tls_config, server_name) {
        Ok(c) => c,
        Err(e) => {
            crate::log::write(&format!("TLS connection init error: {e}"));
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };

    let mut tls_stream = rustls::StreamOwned::new(conn, tcp_stream);

    // 1. Send CONNECT to receiver-0
    let connect_msg = CastMessage::new_string(
        "sender-0",
        "receiver-0",
        NS_CONNECTION,
        r#"{"type":"CONNECT"}"#,
    );
    if let Err(e) = write_cast_message(&mut tls_stream, &connect_msg) {
        crate::log::write(&format!("Failed to send CONNECT to receiver: {e}"));
        is_active.store(false, Ordering::Relaxed);
        return;
    }

    // 2. Launch Default Media Receiver
    let mut req_id = 1u64;
    let launch_payload = serde_json::json!({
        "type": "LAUNCH",
        "appId": DEFAULT_MEDIA_RECEIVER_APP_ID,
        "requestId": req_id
    })
    .to_string();

    let launch_msg = CastMessage::new_string(
        "sender-0",
        "receiver-0",
        NS_RECEIVER,
        launch_payload,
    );
    if let Err(e) = write_cast_message(&mut tls_stream, &launch_msg) {
        crate::log::write(&format!("Failed to send LAUNCH to receiver: {e}"));
        is_active.store(false, Ordering::Relaxed);
        return;
    }

    // 3. Wait for RECEIVER_STATUS to extract transportId and sessionId
    let mut transport_id = String::new();
    let mut session_id = String::new();
    let launch_start = Instant::now();

    while launch_start.elapsed() < Duration::from_secs(10) {
        match read_cast_message(&mut tls_stream) {
            Ok(msg) => {
                if msg.namespace == NS_HEARTBEAT {
                    if let Some(ref text) = msg.payload_utf8 {
                        if text.contains("PING") {
                            let pong = CastMessage::new_string(
                                "sender-0",
                                "receiver-0",
                                NS_HEARTBEAT,
                                r#"{"type":"PONG"}"#,
                            );
                            let _ = write_cast_message(&mut tls_stream, &pong);
                        }
                    }
                } else if msg.namespace == NS_RECEIVER {
                    if let Some(ref text) = msg.payload_utf8 {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(apps) = val["status"]["applications"].as_array() {
                                for app in apps {
                                    if app["appId"] == DEFAULT_MEDIA_RECEIVER_APP_ID {
                                        if let Some(tid) = app["transportId"].as_str() {
                                            transport_id = tid.to_string();
                                        }
                                        if let Some(sid) = app["sessionId"].as_str() {
                                            session_id = sid.to_string();
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                crate::log::write(&format!("Error reading initial receiver status: {e}"));
                break;
            }
        }
        if !transport_id.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    if transport_id.is_empty() {
        crate::log::write("Timed out waiting for Cast Default Media Receiver launch");
        is_active.store(false, Ordering::Relaxed);
        return;
    }

    crate::log::write(&format!(
        "Cast receiver launched: transportId={transport_id}, sessionId={session_id}"
    ));

    // 4. Send CONNECT to transportId
    let transport_connect = CastMessage::new_string(
        "sender-0",
        &transport_id,
        NS_CONNECTION,
        r#"{"type":"CONNECT"}"#,
    );
    if let Err(e) = write_cast_message(&mut tls_stream, &transport_connect) {
        crate::log::write(&format!("Failed to connect to transport: {e}"));
        is_active.store(false, Ordering::Relaxed);
        return;
    }

    if let Ok(mut st) = state.lock() {
        *st = CastState::Idle;
    }

    // 5. Main event & heartbeat loop
    let media_session_id = Arc::new(AtomicI64::new(-1));
    let mut last_ping = Instant::now();

    while is_active.load(Ordering::Relaxed)
        && !crate::SHUTDOWN.load(Ordering::Relaxed)
    {
        // Process outgoing commands
        while let Ok(action) = rx.try_recv() {
            req_id += 1;
            match action {
                CastAction::Load {
                    stream_url,
                    mime_type,
                    title,
                    artist,
                    album,
                    art_url,
                    position: start_pos,
                } => {
                    let load_json = serde_json::json!({
                        "type": "LOAD",
                        "requestId": req_id,
                        "sessionId": session_id,
                        "media": {
                            "contentId": stream_url,
                            "streamType": "BUFFERED",
                            "contentType": mime_type,
                            "metadata": {
                                "metadataType": 3,
                                "title": title,
                                "artist": artist,
                                "albumName": album,
                                "images": [{"url": art_url}]
                            }
                        },
                        "autoplay": true,
                        "currentTime": start_pos
                    });
                    let load_msg = CastMessage::new_string(
                        "sender-0",
                        &transport_id,
                        NS_MEDIA,
                        load_json.to_string(),
                    );
                    let _ = write_cast_message(&mut tls_stream, &load_msg);
                    if let Ok(mut st) = state.lock() {
                        *st = CastState::Buffering;
                    }
                    if let Ok(mut p) = position.lock() {
                        *p = start_pos;
                    }
                    if let Ok(mut t) = last_position_update.lock() {
                        *t = Instant::now();
                    }
                }
                CastAction::Play => {
                    let ms_id = media_session_id.load(Ordering::Relaxed);
                    if ms_id >= 0 {
                        let play_json = serde_json::json!({
                            "type": "PLAY",
                            "requestId": req_id,
                            "mediaSessionId": ms_id
                        });
                        let msg = CastMessage::new_string(
                            "sender-0",
                            &transport_id,
                            NS_MEDIA,
                            play_json.to_string(),
                        );
                        let _ = write_cast_message(&mut tls_stream, &msg);
                    }
                }
                CastAction::Pause => {
                    let ms_id = media_session_id.load(Ordering::Relaxed);
                    if ms_id >= 0 {
                        let pause_json = serde_json::json!({
                            "type": "PAUSE",
                            "requestId": req_id,
                            "mediaSessionId": ms_id
                        });
                        let msg = CastMessage::new_string(
                            "sender-0",
                            &transport_id,
                            NS_MEDIA,
                            pause_json.to_string(),
                        );
                        let _ = write_cast_message(&mut tls_stream, &msg);
                    }
                }
                CastAction::Seek(seek_pos) => {
                    let ms_id = media_session_id.load(Ordering::Relaxed);
                    if ms_id >= 0 {
                        let seek_json = serde_json::json!({
                            "type": "SEEK",
                            "requestId": req_id,
                            "mediaSessionId": ms_id,
                            "currentTime": seek_pos
                        });
                        let msg = CastMessage::new_string(
                            "sender-0",
                            &transport_id,
                            NS_MEDIA,
                            seek_json.to_string(),
                        );
                        let _ = write_cast_message(&mut tls_stream, &msg);
                        if let Ok(mut p) = position.lock() {
                            *p = seek_pos;
                        }
                        if let Ok(mut t) = last_position_update.lock() {
                            *t = Instant::now();
                        }
                    }
                }
                CastAction::Stop => {
                    let ms_id = media_session_id.load(Ordering::Relaxed);
                    if ms_id >= 0 {
                        let stop_json = serde_json::json!({
                            "type": "STOP",
                            "requestId": req_id,
                            "mediaSessionId": ms_id
                        });
                        let msg = CastMessage::new_string(
                            "sender-0",
                            &transport_id,
                            NS_MEDIA,
                            stop_json.to_string(),
                        );
                        let _ = write_cast_message(&mut tls_stream, &msg);
                    }
                }
                CastAction::SetVolume(vol) => {
                    let vol_clamped = vol.clamp(0.0, 1.0);
                    let vol_json = serde_json::json!({
                        "type": "SET_VOLUME",
                        "requestId": req_id,
                        "volume": {
                            "level": vol_clamped
                        }
                    });
                    let msg = CastMessage::new_string(
                        "sender-0",
                        "receiver-0",
                        NS_RECEIVER,
                        vol_json.to_string(),
                    );
                    let _ = write_cast_message(&mut tls_stream, &msg);
                }
                CastAction::Disconnect => {
                    if !session_id.is_empty() {
                        let stop_payload = serde_json::json!({
                            "type": "STOP",
                            "requestId": req_id,
                            "sessionId": session_id,
                        });
                        let stop_msg = CastMessage::new_string(
                            "sender-0",
                            "receiver-0",
                            NS_RECEIVER,
                            stop_payload.to_string(),
                        );
                        let _ = write_cast_message(&mut tls_stream, &stop_msg);
                    }
                    let close_transport = CastMessage::new_string(
                        "sender-0",
                        &transport_id,
                        NS_CONNECTION,
                        r#"{"type":"CLOSE"}"#,
                    );
                    let _ = write_cast_message(&mut tls_stream, &close_transport);
                    let close_receiver = CastMessage::new_string(
                        "sender-0",
                        "receiver-0",
                        NS_CONNECTION,
                        r#"{"type":"CLOSE"}"#,
                    );
                    let _ = write_cast_message(&mut tls_stream, &close_receiver);
                    thread::sleep(Duration::from_millis(50));
                    is_active.store(false, Ordering::Relaxed);
                    return;
                }
            }
        }

        // Heartbeat ping every 5 seconds
        if last_ping.elapsed() >= Duration::from_secs(5) {
            let ping = CastMessage::new_string(
                "sender-0",
                "receiver-0",
                NS_HEARTBEAT,
                r#"{"type":"PING"}"#,
            );
            if write_cast_message(&mut tls_stream, &ping).is_err() {
                crate::log::write("Failed to send Cast heartbeat PING; connection dropped");
                break;
            }
            last_ping = Instant::now();
        }

        // Read incoming messages
        match read_cast_message(&mut tls_stream) {
            Ok(msg) => {
                if msg.namespace == NS_HEARTBEAT {
                    if let Some(ref text) = msg.payload_utf8 {
                        if text.contains("PING") {
                            let pong = CastMessage::new_string(
                                "sender-0",
                                &msg.source_id,
                                NS_HEARTBEAT,
                                r#"{"type":"PONG"}"#,
                            );
                            let _ = write_cast_message(&mut tls_stream, &pong);
                        }
                    }
                } else if msg.namespace == NS_MEDIA {
                    if let Some(ref text) = msg.payload_utf8 {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(msg_type) = val.get("type").and_then(|t| t.as_str()) {
                                if msg_type == "LOAD_FAILED" {
                                    crate::log::write(&format!("Cast LOAD_FAILED: {text}"));
                                }
                            }
                            if let Some(statuses) = val["status"].as_array() {
                                if let Some(media_status) = statuses.first() {
                                    if let Some(ms_id) = media_status["mediaSessionId"].as_i64() {
                                        media_session_id.store(ms_id, Ordering::Relaxed);
                                    }

                                    if let Some(pos) = media_status["currentTime"].as_f64() {
                                        if let Ok(mut p) = position.lock() {
                                            *p = pos;
                                        }
                                        if let Ok(mut t) = last_position_update.lock() {
                                            *t = Instant::now();
                                        }
                                    }

                                    let player_state = media_status["playerState"]
                                        .as_str()
                                        .unwrap_or("IDLE");
                                    let idle_reason = media_status["idleReason"].as_str();

                                    if let Ok(mut st) = state.lock() {
                                        *st = match player_state {
                                            "PLAYING" => CastState::Playing,
                                            "PAUSED" => CastState::Paused,
                                            "BUFFERING" => CastState::Buffering,
                                            _ => CastState::Idle,
                                        };
                                    }

                                    if player_state == "IDLE" {
                                        if idle_reason == Some("FINISHED") {
                                            crate::log::write("Cast track playback finished (EOF)");
                                            on_track_finished();
                                        } else if let Some(reason) = idle_reason {
                                            crate::log::write(&format!("Cast media IDLE reason: {reason}"));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                crate::log::write(&format!("Cast connection read error: {e}"));
                break;
            }
        }
    }

    is_active.store(false, Ordering::Relaxed);
    if let Ok(mut st) = state.lock() {
        *st = CastState::Idle;
    }
    crate::log::write("Cast session worker exited");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_tls_config_creates_valid_verifier() {
        let config = make_tls_config().expect("valid tls config");
        assert!(config.alpn_protocols.is_empty());
    }

    #[test]
    fn test_cast_message_framing() {
        let msg = CastMessage::new_string(
            "sender-0",
            "receiver-0",
            NS_CONNECTION,
            r#"{"type":"CONNECT"}"#,
        );
        let mut buf = Vec::new();
        write_cast_message(&mut buf, &msg).expect("write_cast_message");
        assert!(buf.len() > 4);

        let mut slice = &buf[..];
        let read = read_cast_message(&mut slice).expect("read_cast_message");
        assert_eq!(read.source_id, "sender-0");
        assert_eq!(read.destination_id, "receiver-0");
        assert_eq!(read.namespace, NS_CONNECTION);
    }
}
