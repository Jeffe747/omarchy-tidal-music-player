use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
struct TrackStreamInfo {
    stream_url: String,
    mime_type: String,
}

pub struct StreamProxy {
    port: u16,
    listener: TcpListener,
    tracks: Arc<Mutex<HashMap<u64, TrackStreamInfo>>>,
    shutdown: Arc<AtomicBool>,
}

impl StreamProxy {
    pub fn new() -> Result<Self, String> {
        let listener = TcpListener::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind stream proxy port: {e}"))?;
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
            tracks: Arc::new(Mutex::new(HashMap::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn register_track(&self, track_id: u64, stream_url: String, mime_type: String) {
        if let Ok(mut tracks) = self.tracks.lock() {
            tracks.clear();
            tracks.insert(track_id, TrackStreamInfo { stream_url, mime_type });
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

    pub fn stream_url_for_target(&self, track_id: u64, target_ip: &str) -> String {
        let local_ip = self.resolve_local_ip_for_target(target_ip);
        format!("http://{}:{}/stream/{}", local_ip, self.port, track_id)
    }

    pub fn start(self: Arc<Self>) {
        thread::spawn(move || {
            while !self.shutdown.load(Ordering::Relaxed)
                && !crate::SHUTDOWN.load(Ordering::Relaxed)
            {
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        let tracks_ref = Arc::clone(&self.tracks);
                        thread::spawn(move || {
                            handle_http_client(stream, tracks_ref);
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

fn handle_http_client(
    mut stream: TcpStream,
    tracks: Arc<Mutex<HashMap<u64, TrackStreamInfo>>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
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

    // Extract track_id from /stream/<track_id>
    let track_id: u64 = match path.strip_prefix("/stream/") {
        Some(id_str) => match id_str.split('?').next().unwrap_or("").parse() {
            Ok(id) => id,
            Err(_) => {
                let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
                return;
            }
        },
        None => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

    let track_info = {
        let guard = match tracks.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.get(&track_id).cloned()
    };

    let track_info = match track_info {
        Some(info) => info,
        None => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

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
    let mut req = ureq::get(&track_info.stream_url);
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
        .unwrap_or(&track_info.mime_type)
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
        Connection: keep-alive\r\n"
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

        // Test OPTIONS preflight
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect to proxy");
        client
            .write_all(b"OPTIONS /stream/42 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();

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
}
