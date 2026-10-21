use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct RegisteredStream {
    pub token: String,
    pub track_id: u64,
    pub stream_url: String,
    pub mime_type: String,
    pub receiver_ip: String,
}

pub struct StreamProxy {
    port: u16,
    listener: TcpListener,
    streams: Arc<Mutex<HashMap<String, RegisteredStream>>>,
    active_workers: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
}

struct WorkerBudgetGuard {
    counter: Arc<AtomicUsize>,
}

impl WorkerBudgetGuard {
    fn try_acquire(counter: &Arc<AtomicUsize>) -> Option<Self> {
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            if current >= StreamProxy::MAX_ACTIVE_WORKERS {
                return None;
            }
            match counter.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(Self {
                        counter: Arc::clone(counter),
                    })
                }
                Err(actual) => current = actual,
            }
        }
    }
}

impl Drop for WorkerBudgetGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

fn generate_token() -> String {
    let mut buf = [0u8; 16];
    let mut filled = false;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            filled = true;
        }
    }
    if !filled {
        let _ = rustls::crypto::ring::default_provider()
            .secure_random
            .fill(&mut buf);
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

fn is_authorized_peer(peer_addr: &SocketAddr, receiver_ip: &str) -> bool {
    let peer_ip = peer_addr.ip();
    if let Ok(expected_ip) = receiver_ip.parse::<IpAddr>() {
        let peer_canonical = match peer_ip {
            IpAddr::V4(v4) => IpAddr::V4(v4),
            IpAddr::V6(v6) => {
                if let Some(v4) = v6.to_ipv4_mapped() {
                    IpAddr::V4(v4)
                } else {
                    IpAddr::V6(v6)
                }
            }
        };
        let expected_canonical = match expected_ip {
            IpAddr::V4(v4) => IpAddr::V4(v4),
            IpAddr::V6(v6) => {
                if let Some(v4) = v6.to_ipv4_mapped() {
                    IpAddr::V4(v4)
                } else {
                    IpAddr::V6(v6)
                }
            }
        };
        peer_canonical == expected_canonical
    } else {
        peer_ip.to_string() == receiver_ip
    }
}

impl StreamProxy {
    pub const DEFAULT_PORT: u16 = 36420;
    pub const PORT_RANGE_END: u16 = 36425;
    pub const MAX_ACTIVE_WORKERS: usize = 4;

    pub fn new() -> Result<Self, String> {
        let mut listener = None;
        for port in Self::DEFAULT_PORT..=Self::PORT_RANGE_END {
            if let Ok(l) = TcpListener::bind(("0.0.0.0", port)) {
                listener = Some(l);
                break;
            }
        }
        let listener = match listener {
            Some(l) => l,
            None => TcpListener::bind("0.0.0.0:0")
                .map_err(|e| format!("Failed to bind stream proxy port: {e}"))?,
        };
        let port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get proxy local address: {e}"))?
            .port();

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set proxy listener nonblocking: {e}"))?;

        Ok(Self {
            port,
            listener,
            streams: Arc::new(Mutex::new(HashMap::new())),
            active_workers: Arc::new(AtomicUsize::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn active_workers(&self) -> usize {
        self.active_workers.load(Ordering::Acquire)
    }

    /// Registers a track stream with an unguessable capability token and binds access
    /// exclusively to the target receiver IP address.
    pub fn register_stream(
        &self,
        track_id: u64,
        stream_url: String,
        mime_type: String,
        receiver_ip: &str,
    ) -> String {
        let token = generate_token();
        let local_ip = self.resolve_local_ip_for_target(receiver_ip);
        let registered = RegisteredStream {
            token: token.clone(),
            track_id,
            stream_url,
            mime_type,
            receiver_ip: receiver_ip.to_string(),
        };
        if let Ok(mut streams) = self.streams.lock() {
            // Revoke any previously active stream capability immediately on track transition
            streams.clear();
            streams.insert(token.clone(), registered);
        }
        format!("http://{}:{}/stream/{}/{}", local_ip, self.port, token, track_id)
    }

    pub fn clear_streams(&self) {
        if let Ok(mut streams) = self.streams.lock() {
            streams.clear();
        }
    }

    pub fn resolve_local_ip_for_target(&self, target_ip: &str) -> String {
        // Connect a UDP socket to target_ip to find the outbound network interface IP
        let target_addr = format!("{}:8009", target_ip);
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(&target_addr).is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    let ip = local_addr.ip().to_string();
                    if ip != "0.0.0.0" && ip != "127.0.0.1" {
                        return ip;
                    }
                }
            }
        }
        "127.0.0.1".to_string()
    }

    pub fn start(self: Arc<Self>) {
        thread::spawn(move || {
            while !self.shutdown.load(Ordering::Relaxed)
                && !crate::SHUTDOWN.load(Ordering::Relaxed)
            {
                match self.listener.accept() {
                    Ok((mut stream, peer_addr)) => {
                        // 1. Pre-spawn receiver authorization gate: reject unauthenticated LAN peers
                        // or idle-state connections before allocating any worker thread.
                        let (has_streams, is_authorized) = match self.streams.lock() {
                            Ok(guard) => {
                                let has = !guard.is_empty();
                                let auth = guard
                                    .values()
                                    .any(|s| is_authorized_peer(&peer_addr, &s.receiver_ip));
                                (has, auth)
                            }
                            Err(_) => (false, false),
                        };

                        if !has_streams {
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                            let _ = stream.write_all(
                                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            );
                            continue;
                        }

                        if !is_authorized {
                            crate::log::write(&format!(
                                "StreamProxy: rejected unauthorized connection from {} before spawning worker",
                                peer_addr.ip()
                            ));
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                            let _ = stream.write_all(
                                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            );
                            continue;
                        }

                        // 2. Enforce strict concurrent worker budget before spawning a native thread.
                        let worker_guard = match WorkerBudgetGuard::try_acquire(&self.active_workers) {
                            Some(guard) => guard,
                            None => {
                                let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                                let _ = stream.write_all(
                                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                );
                                continue;
                            }
                        };

                        let streams_ref = Arc::clone(&self.streams);
                        thread::spawn(move || {
                            let _guard = worker_guard;
                            handle_http_client(stream, peer_addr, streams_ref);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        crate::log::write(&format!("StreamProxy accept error: {e}"));
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub fn check_cast_ports_blocked(proxy_port: u16) -> bool {
    if let Ok(val) = std::env::var("TIDAL_CAST_FIREWALL_BLOCKED") {
        if val == "1" || val.eq_ignore_ascii_case("true") {
            return true;
        }
        if val == "0" || val.eq_ignore_ascii_case("false") {
            return false;
        }
    }

    // Check UFW configuration files directly without spawning privileged commands
    if let Ok(ufw_conf) = std::fs::read_to_string("/etc/ufw/ufw.conf") {
        let ufw_enabled = ufw_conf.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == "ENABLED=yes"
                || trimmed == "ENABLED=\"yes\""
                || trimmed == "ENABLED='yes'"
        });

        if ufw_enabled {
            // If DEFAULT_INPUT_POLICY is ACCEPT, inbound traffic is permitted
            if let Ok(default_ufw) = std::fs::read_to_string("/etc/default/ufw") {
                let is_accept = default_ufw.lines().any(|line| {
                    let trimmed = line.trim();
                    trimmed.starts_with("DEFAULT_INPUT_POLICY=\"ACCEPT\"")
                        || trimmed.starts_with("DEFAULT_INPUT_POLICY='ACCEPT'")
                        || trimmed.starts_with("DEFAULT_INPUT_POLICY=ACCEPT")
                });
                if is_accept {
                    return false;
                }
            }

            let port_str = proxy_port.to_string();

            // Check if /etc/ufw/user.rules allows proxy_port or 36420
            if let Ok(rules) = std::fs::read_to_string("/etc/ufw/user.rules") {
                if rules.contains(&port_str) || rules.contains("36420") {
                    return false;
                }
            }

            return true;
        }
    }

    false
}

fn handle_http_client(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    streams: Arc<Mutex<HashMap<String, RegisteredStream>>>,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];

    // Extract unguessable capability token and track_id from /stream/<token>/<track_id>
    let path_no_query = path.split('?').next().unwrap_or("");
    let rest = match path_no_query.strip_prefix("/stream/") {
        Some(r) => r,
        None => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

    let mut segments = rest.split('/');
    let token = segments.next().unwrap_or("");
    let track_id_str = segments.next().unwrap_or("");
    if token.is_empty() || track_id_str.is_empty() || segments.next().is_some() {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    }

    let requested_track_id: u64 = match track_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

    let stream_info = {
        let guard = match streams.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.get(token).cloned()
    };

    let stream_info = match stream_info {
        Some(info) if info.track_id == requested_track_id => info,
        _ => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

    // Security: Only the authorized receiver IP may access this stream
    if !is_authorized_peer(&peer_addr, &stream_info.receiver_ip) {
        crate::log::write(&format!(
            "StreamProxy: unauthorized connection from {} attempted access to stream bound to {}",
            peer_addr.ip(),
            stream_info.receiver_ip
        ));
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n");
        return;
    }

    // CORS preflight OPTIONS request
    if method == "OPTIONS" {
        let response = "HTTP/1.1 200 OK\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
            Access-Control-Allow-Headers: *\r\n\
            Access-Control-Allow-Private-Network: true\r\n\
            Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n\
            Access-Control-Max-Age: 86400\r\n\
            Content-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        return;
    }

    if method != "GET" && method != "HEAD" {
        let response = "HTTP/1.1 405 Method Not Allowed\r\n\
            Content-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Extract Range header
    let mut range_header = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some(val) = line.strip_prefix("Range: ") {
            range_header = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("range: ") {
            range_header = Some(val.trim().to_string());
        }
    }

    // Request from Tidal CDN without Origin header to bypass CDN blocking
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(30))
        .build();
    let mut req = agent.get(&stream_info.stream_url);
    if let Some(ref range) = range_header {
        req = req.set("Range", range);
    }

    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(206, r)) => r,
        Err(e) => {
            crate::log::write(&format!("StreamProxy upstream fetch error: {e}"));
            let _ = stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

    let status_code = resp.status();
    let status_text = if status_code == 206 { "Partial Content" } else { "OK" };

    let content_type = resp
        .header("Content-Type")
        .unwrap_or(&stream_info.mime_type)
        .to_string();

    let content_length = resp.header("Content-Length").map(str::to_string);
    let content_range = resp.header("Content-Range").map(str::to_string);

    let mut headers = format!(
        "HTTP/1.1 {status_code} {status_text}\r\n\
        Content-Type: {content_type}\r\n\
        Accept-Ranges: bytes\r\n\
        Cache-Control: no-cache, no-store, must-revalidate\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
        Access-Control-Allow-Headers: *\r\n\
        Access-Control-Allow-Private-Network: true\r\n\
        Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n\
        Connection: close\r\n"
    );

    if let Some(len) = content_length {
        headers.push_str(&format!("Content-Length: {len}\r\n"));
    }
    if let Some(cr) = content_range {
        headers.push_str(&format!("Content-Range: {cr}\r\n"));
    }
    headers.push_str("\r\n");

    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }

    if method == "HEAD" {
        let _ = stream.flush();
        return;
    }

    let mut reader = resp.into_reader();
    let mut chunk = [0u8; 16384];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read_bytes) => {
                if stream.write_all(&chunk[..read_bytes]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_proxy_bind_and_options() {
        let proxy = Arc::new(StreamProxy::new().expect("Failed to create StreamProxy"));
        let port = proxy.port();
        assert!(port > 0);

        Arc::clone(&proxy).start();

        // Register a stream for localhost receiver
        let stream_url = proxy.register_stream(
            42,
            "http://example.com/audio.flac".into(),
            "audio/flac".into(),
            "127.0.0.1",
        );
        let path = stream_url.find("/stream/").map(|idx| &stream_url[idx..]).expect("valid stream path");

        // Test OPTIONS preflight from authorized IP
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect to proxy");
        let req = format!("OPTIONS {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        client.write_all(req.as_bytes()).unwrap();

        let mut resp = [0u8; 1024];
        let n = client.read(&mut resp).unwrap();
        let resp_str = String::from_utf8_lossy(&resp[..n]);

        assert!(resp_str.contains("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("Access-Control-Allow-Origin: *"));
        assert!(resp_str.contains("Access-Control-Allow-Methods: GET, HEAD, OPTIONS"));
        assert!(resp_str.contains("Access-Control-Allow-Private-Network: true"));
        assert!(resp_str.contains("Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges"));

        proxy.stop();
    }

    #[test]
    fn test_stream_proxy_authorization_and_tokens() {
        let proxy = Arc::new(StreamProxy::new().expect("Failed to create StreamProxy"));
        let port = proxy.port();
        Arc::clone(&proxy).start();

        // 1. Unregistered token or plain track ID returns 404
        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .write_all(b"GET /stream/42 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let mut resp = [0u8; 512];
        let n = client.read(&mut resp).unwrap();
        assert!(String::from_utf8_lossy(&resp[..n]).contains("HTTP/1.1 404 Not Found"));

        // 2. Register stream for 127.0.0.1, then verify mismatched track_id returns 404
        let stream1_url = proxy.register_stream(
            101,
            "http://example.com/audio1.flac".into(),
            "audio/flac".into(),
            "127.0.0.1",
        );
        let stream1_path = stream1_url.find("/stream/").map(|idx| &stream1_url[idx..]).unwrap();
        let wrong_id_path = stream1_path.replacen("/101", "/999", 1);
        let mut client_wrong_id = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req_wrong_id = format!("GET {wrong_id_path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        client_wrong_id.write_all(req_wrong_id.as_bytes()).unwrap();
        let mut resp_wrong_id = [0u8; 512];
        let n_wrong = client_wrong_id.read(&mut resp_wrong_id).unwrap();
        assert!(String::from_utf8_lossy(&resp_wrong_id[..n_wrong]).contains("HTTP/1.1 404 Not Found"));

        // 3. Registering a new stream on track transition immediately revokes the previous token
        let _stream2_url = proxy.register_stream(
            99,
            "http://example.com/audio2.flac".into(),
            "audio/flac".into(),
            "127.0.0.1",
        );
        let mut client_revoked = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req_revoked = format!("OPTIONS {stream1_path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        client_revoked.write_all(req_revoked.as_bytes()).unwrap();
        let mut resp_revoked = [0u8; 512];
        let n_revoked = client_revoked.read(&mut resp_revoked).unwrap();
        assert!(String::from_utf8_lossy(&resp_revoked[..n_revoked]).contains("HTTP/1.1 404 Not Found"));

        // 4. Register for a remote IP: connecting from 127.0.0.1 must be rejected with 403 Forbidden
        let remote_stream_url = proxy.register_stream(
            99,
            "http://example.com/audio.flac".into(),
            "audio/flac".into(),
            "192.168.1.200",
        );
        let remote_path = remote_stream_url.find("/stream/").map(|idx| &remote_stream_url[idx..]).unwrap();
        let mut client2 = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req2 = format!("GET {remote_path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        client2.write_all(req2.as_bytes()).unwrap();
        let mut resp2 = [0u8; 512];
        let n2 = client2.read(&mut resp2).unwrap();
        assert!(String::from_utf8_lossy(&resp2[..n2]).contains("HTTP/1.1 403 Forbidden"));

        // 5. Clear streams revokes previously registered capability
        proxy.clear_streams();
        let mut client3 = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client3.write_all(req2.as_bytes()).unwrap();
        let mut resp3 = [0u8; 512];
        let n3 = client3.read(&mut resp3).unwrap();
        assert!(String::from_utf8_lossy(&resp3[..n3]).contains("HTTP/1.1 404 Not Found"));

        proxy.stop();
    }

    #[test]
    fn test_stream_proxy_connection_budget() {
        let proxy = Arc::new(StreamProxy::new().expect("Failed to create StreamProxy"));
        let port = proxy.port();
        Arc::clone(&proxy).start();

        // Register a stream for localhost so connections pass the pre-spawn receiver IP check
        proxy.register_stream(
            1,
            "http://example.com/audio.flac".into(),
            "audio/flac".into(),
            "127.0.0.1",
        );

        // Hold MAX_ACTIVE_WORKERS connections open without sending any bytes (blocking in worker read())
        let mut held_clients = Vec::new();
        for _ in 0..StreamProxy::MAX_ACTIVE_WORKERS {
            let c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            held_clients.push(c);
        }

        // Wait briefly for the accept loop to spawn the 4 workers
        for _ in 0..50 {
            if proxy.active_workers() == StreamProxy::MAX_ACTIVE_WORKERS {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(proxy.active_workers(), StreamProxy::MAX_ACTIVE_WORKERS);

        // 5th connection must be rejected immediately in the accept loop with 503 Service Unavailable
        let mut overflow_client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        overflow_client
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let mut resp = [0u8; 512];
        let n = overflow_client.read(&mut resp).unwrap();
        assert!(
            String::from_utf8_lossy(&resp[..n]).contains("HTTP/1.1 503 Service Unavailable"),
            "Expected 503 Service Unavailable when worker budget is exhausted"
        );
        assert_eq!(proxy.active_workers(), StreamProxy::MAX_ACTIVE_WORKERS);

        // Drop held clients so workers exit and release their budget guards
        drop(held_clients);
        for _ in 0..50 {
            if proxy.active_workers() == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(proxy.active_workers(), 0);

        proxy.stop();
    }

    #[test]
    fn test_check_cast_ports_blocked_env_override() {
        std::env::set_var("TIDAL_CAST_FIREWALL_BLOCKED", "1");
        assert!(check_cast_ports_blocked(36420));
        std::env::set_var("TIDAL_CAST_FIREWALL_BLOCKED", "0");
        assert!(!check_cast_ports_blocked(36420));
        std::env::remove_var("TIDAL_CAST_FIREWALL_BLOCKED");
    }
}
