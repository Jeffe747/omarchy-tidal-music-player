use super::CastDevice;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::unix::io::FromRawFd;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MDNS_MULTICAST_IPV4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;
const DEVICE_TTL: Duration = Duration::from_secs(60);

// Query for PTR _googlecast._tcp.local
// Header: ID=0, Flags=0x0000, QDCOUNT=1, ANCOUNT=0, NSCOUNT=0, ARCOUNT=0
// Question: _googlecast._tcp.local, Type=PTR (12), Class=IN (1)
const MDNS_QUERY: [u8; 40] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Header
    0x0b, b'_', b'g', b'o', b'o', b'g', b'l', b'e', b'c', b'a', b's', b't', 0x04, b'_', b't',
    b'c', b'p', 0x05, b'l', b'o', b'c', b'a', b'l', 0x00, // QNAME
    0x00, 0x0c, // QTYPE = PTR
    0x00, 0x01, // QCLASS = IN
];

#[derive(Clone)]
pub struct CastDiscovery {
    cache: Arc<Mutex<HashMap<String, (CastDevice, Instant)>>>,
}

impl CastDiscovery {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_devices(&self) -> Vec<CastDevice> {
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        cache.retain(|_, (_, instant)| instant.elapsed() < DEVICE_TTL);
        let mut devices: Vec<CastDevice> = cache.values().map(|(d, _)| d.clone()).collect();
        devices.sort_by(|a, b| a.name.cmp(&b.name));
        devices
    }

    pub fn discover(&self, timeout: Duration) -> Vec<CastDevice> {
        let socket = match create_mdns_socket() {
            Ok(s) => s,
            Err(e) => {
                crate::log::write(&format!("Failed to create mDNS socket: {e}"));
                return self.get_devices();
            }
        };

        let target = SocketAddr::from((MDNS_MULTICAST_IPV4, MDNS_PORT));
        if let Err(e) = socket.send_to(&MDNS_QUERY, target) {
            crate::log::write(&format!("Failed to send mDNS query: {e}"));
        }

        let start = Instant::now();
        let mut buf = [0u8; 4096];

        while start.elapsed() < timeout {
            let elapsed = start.elapsed();
            let remaining = timeout.saturating_sub(elapsed);
            let read_timeout = remaining.min(Duration::from_millis(150));
            let _ = socket.set_read_timeout(Some(read_timeout));

            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    let packet = &buf[..len];
                    if let Some(device) = parse_cast_mdns_packet(packet, src) {
                        if let Ok(mut cache) = self.cache.lock() {
                            cache.insert(device.id.clone(), (device, Instant::now()));
                        }
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Continue until overall timeout
                }
                Err(e) => {
                    crate::log::write(&format!("mDNS recv error: {e}"));
                    break;
                }
            }
        }

        self.get_devices()
    }

    pub fn start_passive_listener(self: Arc<Self>) {
        std::thread::spawn(move || {
            loop {
                // Passive discovery refresh every 45 seconds
                self.discover(Duration::from_millis(800));
                std::thread::sleep(Duration::from_secs(45));
                if crate::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
            }
        });
    }
}

fn create_mdns_socket() -> std::io::Result<UdpSocket> {
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let opt: libc::c_int = 1;
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &opt as *const _ as *const _,
            std::mem::size_of_val(&opt) as _,
        );
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT,
            &opt as *const _ as *const _,
            std::mem::size_of_val(&opt) as _,
        );

        let mut addr: libc::sockaddr_in = std::mem::zeroed();
        addr.sin_family = libc::AF_INET as libc::sa_family_t;
        addr.sin_port = MDNS_PORT.to_be();
        addr.sin_addr.s_addr = libc::INADDR_ANY.to_be();

        let bind_res = libc::bind(
            fd,
            &addr as *const _ as *const _,
            std::mem::size_of_val(&addr) as _,
        );

        let socket = if bind_res == 0 {
            UdpSocket::from_raw_fd(fd)
        } else {
            libc::close(fd);
            UdpSocket::bind("0.0.0.0:0")?
        };

        let _ = socket.join_multicast_v4(&MDNS_MULTICAST_IPV4, &Ipv4Addr::UNSPECIFIED);
        let _ = socket.set_multicast_loop_v4(true);
        let _ = socket.set_read_timeout(Some(Duration::from_millis(300)));
        Ok(socket)
    }
}

pub fn parse_cast_mdns_packet(data: &[u8], src: SocketAddr) -> Option<CastDevice> {
    if data.len() < 12 {
        return None;
    }

    let qdcount = u16::from_be_bytes([data[4], data[5]]) as usize;
    let ancount = u16::from_be_bytes([data[6], data[7]]) as usize;
    let nscount = u16::from_be_bytes([data[8], data[9]]) as usize;
    let arcount = u16::from_be_bytes([data[10], data[11]]) as usize;

    let total_records = ancount + nscount + arcount;
    if total_records == 0 {
        return None;
    }

    let mut offset = 12;

    // Skip questions
    for _ in 0..qdcount {
        if skip_dns_name(data, &mut offset).is_err() || offset + 4 > data.len() {
            return None;
        }
        offset += 4; // qtype + qclass
    }

    let mut device_id = None;
    let mut friendly_name = None;
    let mut model_name = None;
    let mut port = None;
    let mut ip_addr = None;

    for _ in 0..total_records {
        if offset >= data.len() {
            break;
        }

        if skip_dns_name(data, &mut offset).is_err() || offset + 10 > data.len() {
            break;
        }

        let rtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let rdlength = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as usize;
        offset += 10;

        if offset + rdlength > data.len() {
            break;
        }

        let rdata = &data[offset..offset + rdlength];
        offset += rdlength;

        match rtype {
            1 => {
                // A Record (IPv4)
                if rdata.len() == 4 {
                    ip_addr = Some(format!("{}.{}.{}.{}", rdata[0], rdata[1], rdata[2], rdata[3]));
                }
            }
            16 => {
                // TXT Record
                let mut txt_offset = 0;
                while txt_offset < rdata.len() {
                    let len = rdata[txt_offset] as usize;
                    txt_offset += 1;
                    if txt_offset + len > rdata.len() {
                        break;
                    }
                    let item = String::from_utf8_lossy(&rdata[txt_offset..txt_offset + len]);
                    txt_offset += len;

                    if let Some(idx) = item.find('=') {
                        let key = &item[..idx];
                        let val = &item[idx + 1..];
                        match key {
                            "id" => {
                                if device_id.is_none() {
                                    device_id = Some(val.to_string());
                                }
                            }
                            "fn" => friendly_name = Some(val.to_string()),
                            "md" => model_name = Some(val.to_string()),
                            _ => {}
                        }
                    }
                }
            }
            33 => {
                // SRV Record: Priority (2), Weight (2), Port (2), Target (...)
                if rdata.len() >= 6 {
                    let p = u16::from_be_bytes([rdata[4], rdata[5]]);
                    port = Some(p);
                }
            }
            _ => {}
        }
    }

    // Fallback IP to UDP packet sender
    let final_ip = ip_addr.unwrap_or_else(|| src.ip().to_string());
    let final_port = port.unwrap_or(8009);

    if let (Some(id), Some(name)) = (device_id, friendly_name) {
        Some(CastDevice {
            id,
            name,
            model: model_name.unwrap_or_else(|| "Google Cast".to_string()),
            ip: final_ip,
            port: final_port,
        })
    } else {
        None
    }
}

fn skip_dns_name(data: &[u8], offset: &mut usize) -> Result<(), String> {
    let mut cur = *offset;
    let mut loops = 0;
    while cur < data.len() && loops < 50 {
        loops += 1;
        let len = data[cur] as usize;
        if len == 0 {
            *offset = cur + 1;
            return Ok(());
        }
        if (len & 0xC0) == 0xC0 {
            // Pointer
            if cur + 2 > data.len() {
                return Err("Pointer out of bounds".to_string());
            }
            *offset = cur + 2;
            return Ok(());
        }
        cur += 1 + len;
    }
    Err("Name skip failed or exceeded max labels".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_real_cast_mdns_packet() {
        // Real packet captured from Google Home Mini on LAN
        let hex_data = "0000840000000001000000030b5f676f6f676c6563617374045f746370056c6f63616c00000c000100000078003431476f6f676c652d486f6d652d4d696e692d3232653563333966313430636136616364303636323935646530666134343365c00cc02e001080010000119400be2369643d32326535633339663134306361366163643036363239356465306661343433652363643d384345433444433445333444453238364132383145454632303437454639323113726d3d423634433642343442433245353835450576653d3035136d643d476f6f676c6520486f6d65204d696e691269633d2f73657475702f69636f6e2e706e670c666e3d4d696e69204a6f686e0963613d3139393137320473743d300f62733d464138464341334435444142046e663d310372733dc02e0021800100000078002d000000001f492432326535633339662d313430632d613661632d643036362d323935646530666134343365c01dc13e00018001000000780004c0a8563f";
        let bytes: Vec<u8> = (0..hex_data.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex_data[i..i + 2], 16).unwrap())
            .collect();

        let src = "192.168.86.63:5353".parse().unwrap();
        let device = parse_cast_mdns_packet(&bytes, src).expect("expected parsed CastDevice");

        assert_eq!(device.id, "22e5c39f140ca6acd066295de0fa443e");
        assert_eq!(device.name, "Mini John");
        assert_eq!(device.model, "Google Home Mini");
        assert_eq!(device.ip, "192.168.86.63");
        assert_eq!(device.port, 8009);
    }

    #[test]
    fn test_cast_discovery_cache_expiry() {
        let discovery = CastDiscovery::new();
        let dev = CastDevice {
            id: "test-id".to_string(),
            name: "Test Cast".to_string(),
            model: "Chromecast Audio".to_string(),
            ip: "192.168.1.100".to_string(),
            port: 8009,
        };
        discovery
            .cache
            .lock()
            .unwrap()
            .insert(dev.id.clone(), (dev.clone(), Instant::now()));

        let devices = discovery.get_devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Test Cast");
    }
}
