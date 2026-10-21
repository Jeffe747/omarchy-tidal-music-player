pub mod client;
pub mod discovery;
pub mod proxy;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CastDevice {
    pub id: String,
    pub name: String,
    pub model: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CastState {
    Idle,
    Connecting,
    Playing,
    Buffering,
    Paused,
}

impl CastState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CastState::Idle => "idle",
            CastState::Connecting => "connecting",
            CastState::Playing => "playing",
            CastState::Buffering => "buffering",
            CastState::Paused => "paused",
        }
    }
}

impl Default for CastState {
    fn default() -> Self {
        CastState::Idle
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastMessage {
    pub source_id: String,
    pub destination_id: String,
    pub namespace: String,
    pub payload_utf8: Option<String>,
    pub payload_binary: Option<Vec<u8>>,
}

fn encode_varint(mut val: u64, buf: &mut Vec<u8>) {
    while val >= 0x80 {
        buf.push(((val & 0x7F) as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

fn encode_string_field(field_no: u32, s: &str, buf: &mut Vec<u8>) {
    let tag = (field_no << 3) | 2;
    encode_varint(tag as u64, buf);
    encode_varint(s.len() as u64, buf);
    buf.extend_from_slice(s.as_bytes());
}

fn encode_varint_field(field_no: u32, val: u64, buf: &mut Vec<u8>) {
    let tag = (field_no << 3) | 0;
    encode_varint(tag as u64, buf);
    encode_varint(val, buf);
}

fn decode_varint(data: &[u8], offset: &mut usize) -> Result<u64, String> {
    let mut result: u64 = 0;
    let mut shift = 0;
    while *offset < data.len() {
        let b = data[*offset];
        *offset += 1;
        result |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            return Err("Varint overflow in protobuf decoding".to_string());
        }
    }
    Err("Unexpected EOF decoding protobuf varint".to_string())
}

impl CastMessage {
    pub fn new_string(
        source_id: impl Into<String>,
        destination_id: impl Into<String>,
        namespace: impl Into<String>,
        payload_utf8: impl Into<String>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            destination_id: destination_id.into(),
            namespace: namespace.into(),
            payload_utf8: Some(payload_utf8.into()),
            payload_binary: None,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Field 1: protocol_version = 0 (CASTV2_1_0)
        encode_varint_field(1, 0, &mut buf);
        // Field 2: source_id
        encode_string_field(2, &self.source_id, &mut buf);
        // Field 3: destination_id
        encode_string_field(3, &self.destination_id, &mut buf);
        // Field 4: namespace
        encode_string_field(4, &self.namespace, &mut buf);

        if let Some(ref binary) = self.payload_binary {
            // Field 5: payload_type = 1 (BINARY)
            encode_varint_field(5, 1, &mut buf);
            // Field 7: payload_binary
            let tag = (7 << 3) | 2;
            encode_varint(tag as u64, &mut buf);
            encode_varint(binary.len() as u64, &mut buf);
            buf.extend_from_slice(binary);
        } else {
            // Field 5: payload_type = 0 (STRING)
            encode_varint_field(5, 0, &mut buf);
            if let Some(ref utf8) = self.payload_utf8 {
                encode_string_field(6, utf8, &mut buf);
            }
        }
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut offset = 0;
        let mut source_id = String::new();
        let mut destination_id = String::new();
        let mut namespace = String::new();
        let mut payload_utf8 = None;
        let mut payload_binary = None;

        while offset < data.len() {
            let key = decode_varint(data, &mut offset)?;
            let field_num = (key >> 3) as u32;
            let wire_type = key & 0x07;

            match wire_type {
                0 => {
                    let _ = decode_varint(data, &mut offset)?;
                }
                2 => {
                    let len = decode_varint(data, &mut offset)? as usize;
                    let end = match offset.checked_add(len) {
                        Some(e) if e <= data.len() => e,
                        _ => return Err("Protobuf length exceeds buffer bounds".to_string()),
                    };
                    let bytes = &data[offset..end];
                    offset = end;
                    match field_num {
                        2 => source_id = String::from_utf8_lossy(bytes).into_owned(),
                        3 => destination_id = String::from_utf8_lossy(bytes).into_owned(),
                        4 => namespace = String::from_utf8_lossy(bytes).into_owned(),
                        6 => payload_utf8 = Some(String::from_utf8_lossy(bytes).into_owned()),
                        7 => payload_binary = Some(bytes.to_vec()),
                        _ => {}
                    }
                }
                _ => {
                    return Err(format!("Unsupported protobuf wire type: {wire_type}"));
                }
            }
        }

        Ok(Self {
            source_id,
            destination_id,
            namespace,
            payload_utf8,
            payload_binary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cast_message_roundtrip() {
        let msg = CastMessage::new_string(
            "sender-0",
            "receiver-0",
            "urn:x-cast:com.google.cast.tp.connection",
            r#"{"type":"CONNECT"}"#,
        );
        let encoded = msg.to_bytes();
        let decoded = CastMessage::from_bytes(&encoded).expect("failed to decode CastMessage");
        assert_eq!(decoded.source_id, "sender-0");
        assert_eq!(decoded.destination_id, "receiver-0");
        assert_eq!(
            decoded.namespace,
            "urn:x-cast:com.google.cast.tp.connection"
        );
        assert_eq!(decoded.payload_utf8.as_deref(), Some(r#"{"type":"CONNECT"}"#));
        assert_eq!(decoded.payload_binary, None);
    }

    #[test]
    fn test_cast_device_serialization() {
        let device = CastDevice {
            id: "22e5c39f140ca6acd066295de0fa443e".to_string(),
            name: "Mini John".to_string(),
            model: "Google Home Mini".to_string(),
            ip: "192.168.86.63".to_string(),
            port: 8009,
        };
        let json = serde_json::to_string(&device).expect("serialize CastDevice");
        let parsed: CastDevice = serde_json::from_str(&json).expect("deserialize CastDevice");
        assert_eq!(parsed, device);
    }

    #[test]
    fn test_protobuf_overflow_payload() {
        // 0x12 is field 2 (source_id), wire type 2 (length-delimited)
        // 9x 0xff bytes followed by 0x01 encodes u64::MAX
        let payload = vec![0x12, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
        let res = CastMessage::from_bytes(&payload);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            "Protobuf length exceeds buffer bounds"
        );
    }
}
