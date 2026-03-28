//! Binary message protocol with framing and optional compression.
//!
//! Messages use a simple binary format:
//! - Header: message_type (u8) + sequence_number (u32) + payload_length (u16)
//! - Payload: raw bytes
//!
//! All multi-byte integers are encoded in little-endian byte order.
//! No serde dependency -- manual serialization for WASM size.

/// Type identifier for network messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    /// Client wishes to connect.
    Connect = 0,
    /// Client or server signals disconnection.
    Disconnect = 1,
    /// Ping (latency probe).
    Ping = 2,
    /// Pong (latency response).
    Pong = 3,
    /// Full or partial world state from server.
    StateUpdate = 4,
    /// Player input from client.
    Input = 5,
    /// Application-defined custom message.
    Custom = 6,
}

impl MessageType {
    /// Converts a raw u8 to a MessageType, if valid.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Connect),
            1 => Some(Self::Disconnect),
            2 => Some(Self::Ping),
            3 => Some(Self::Pong),
            4 => Some(Self::StateUpdate),
            5 => Some(Self::Input),
            6 => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Header for a framed network message.
///
/// Layout (7 bytes total):
/// - `message_type`: u8  (1 byte)
/// - `sequence`:     u32 (4 bytes, little-endian)
/// - `payload_len`:  u16 (2 bytes, little-endian)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MessageHeader {
    pub message_type: MessageType,
    pub sequence: u32,
    pub payload_len: u16,
}

/// Size of a serialized message header in bytes.
pub const HEADER_SIZE: usize = 7;

/// Maximum payload size (64 KB - 1).
pub const MAX_PAYLOAD_SIZE: usize = u16::MAX as usize;

impl MessageHeader {
    /// Encodes this header into a 7-byte buffer.
    pub fn encode(&self, buf: &mut [u8]) {
        debug_assert!(buf.len() >= HEADER_SIZE);
        buf[0] = self.message_type as u8;
        buf[1..5].copy_from_slice(&self.sequence.to_le_bytes());
        buf[5..7].copy_from_slice(&self.payload_len.to_le_bytes());
    }

    /// Decodes a header from a 7-byte buffer.
    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.len() < HEADER_SIZE {
            return Err(ProtocolError::BufferTooSmall);
        }
        let msg_type = MessageType::from_u8(buf[0])
            .ok_or(ProtocolError::InvalidMessageType(buf[0]))?;
        let sequence = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        let payload_len = u16::from_le_bytes([buf[5], buf[6]]);
        Ok(Self {
            message_type: msg_type,
            sequence,
            payload_len,
        })
    }
}

/// A complete framed network message (header + payload).
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub header: MessageHeader,
    pub payload: Vec<u8>,
}

impl Message {
    /// Creates a new message.
    pub fn new(message_type: MessageType, sequence: u32, payload: Vec<u8>) -> Self {
        let payload_len = payload.len().min(MAX_PAYLOAD_SIZE) as u16;
        Self {
            header: MessageHeader {
                message_type,
                sequence,
                payload_len,
            },
            payload,
        }
    }

    /// Creates a Connect message with an optional client name payload.
    pub fn connect(sequence: u32, name: &str) -> Self {
        Self::new(MessageType::Connect, sequence, name.as_bytes().to_vec())
    }

    /// Creates a Disconnect message.
    pub fn disconnect(sequence: u32) -> Self {
        Self::new(MessageType::Disconnect, sequence, Vec::new())
    }

    /// Creates a Ping message with a timestamp payload (u64 millis).
    pub fn ping(sequence: u32, timestamp_ms: u64) -> Self {
        Self::new(MessageType::Ping, sequence, timestamp_ms.to_le_bytes().to_vec())
    }

    /// Creates a Pong message echoing the ping timestamp.
    pub fn pong(sequence: u32, timestamp_ms: u64) -> Self {
        Self::new(MessageType::Pong, sequence, timestamp_ms.to_le_bytes().to_vec())
    }

    /// Creates a StateUpdate message.
    pub fn state_update(sequence: u32, data: Vec<u8>) -> Self {
        Self::new(MessageType::StateUpdate, sequence, data)
    }

    /// Creates an Input message.
    pub fn input(sequence: u32, data: Vec<u8>) -> Self {
        Self::new(MessageType::Input, sequence, data)
    }

    /// Creates a Custom message.
    pub fn custom(sequence: u32, data: Vec<u8>) -> Self {
        Self::new(MessageType::Custom, sequence, data)
    }
}

/// Errors from protocol encoding/decoding.
#[derive(Clone, Debug, PartialEq)]
pub enum ProtocolError {
    /// Buffer is too small to contain a header or full message.
    BufferTooSmall,
    /// Unrecognized message type byte.
    InvalidMessageType(u8),
    /// Payload length in header doesn't match available data.
    PayloadMismatch { expected: u16, available: usize },
    /// Decompression failed.
    DecompressFailed,
    /// Payload exceeds maximum size.
    PayloadTooLarge,
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProtocolError::BufferTooSmall => write!(f, "buffer too small"),
            ProtocolError::InvalidMessageType(t) => write!(f, "invalid message type: {t}"),
            ProtocolError::PayloadMismatch { expected, available } => {
                write!(f, "payload mismatch: expected {expected}, available {available}")
            }
            ProtocolError::DecompressFailed => write!(f, "decompression failed"),
            ProtocolError::PayloadTooLarge => write!(f, "payload exceeds maximum size"),
        }
    }
}

// ---------------------------------------------------------------------------
// Encode / Decode
// ---------------------------------------------------------------------------

/// Encodes a message into a byte vector (header + payload).
pub fn encode_message(msg: &Message) -> Result<Vec<u8>, ProtocolError> {
    if msg.payload.len() > MAX_PAYLOAD_SIZE {
        return Err(ProtocolError::PayloadTooLarge);
    }
    let total = HEADER_SIZE + msg.payload.len();
    let mut buf = vec![0u8; total];
    msg.header.encode(&mut buf[..HEADER_SIZE]);
    buf[HEADER_SIZE..].copy_from_slice(&msg.payload);
    Ok(buf)
}

/// Decodes a message from a byte slice.
///
/// Returns the decoded message and the number of bytes consumed.
pub fn decode_message(buf: &[u8]) -> Result<(Message, usize), ProtocolError> {
    let header = MessageHeader::decode(buf)?;
    let payload_len = header.payload_len as usize;
    let total = HEADER_SIZE + payload_len;
    if buf.len() < total {
        return Err(ProtocolError::PayloadMismatch {
            expected: header.payload_len,
            available: buf.len() - HEADER_SIZE,
        });
    }
    let payload = buf[HEADER_SIZE..total].to_vec();
    let msg = Message {
        header,
        payload,
    };
    Ok((msg, total))
}

// ---------------------------------------------------------------------------
// Simple compression (RLE-inspired, lightweight)
// ---------------------------------------------------------------------------

/// Simple compression using a run-length encoding scheme.
///
/// This is a lightweight alternative to LZ4 that adds minimal code size.
/// Format: for each run:
///   - If the byte repeats N times (N >= 4): marker 0x00, count (u16 LE), byte
///   - Otherwise: literal bytes prefixed with length (u16 LE) + marker 0x01
///
/// This is optimized for state update data that often has runs of zeros.
pub fn compress_simple(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        // Count run of identical bytes
        let byte = data[i];
        let mut run_len = 1usize;
        while i + run_len < data.len() && data[i + run_len] == byte && run_len < 65535 {
            run_len += 1;
        }

        if run_len >= 4 {
            // Encode as run: marker(0x00) + count(u16 LE) + byte
            out.push(0x00);
            out.extend_from_slice(&(run_len as u16).to_le_bytes());
            out.push(byte);
            i += run_len;
        } else {
            // Collect literal bytes until we hit a run of 4+
            let lit_start = i;
            while i < data.len() {
                // Check if we're at the start of a run >= 4
                if i + 3 < data.len()
                    && data[i] == data[i + 1]
                    && data[i] == data[i + 2]
                    && data[i] == data[i + 3]
                {
                    break;
                }
                i += 1;
                // Cap literal block at 65535 bytes
                if i - lit_start >= 65535 {
                    break;
                }
            }
            let lit_len = i - lit_start;
            out.push(0x01);
            out.extend_from_slice(&(lit_len as u16).to_le_bytes());
            out.extend_from_slice(&data[lit_start..lit_start + lit_len]);
        }
    }

    out
}

/// Decompresses data produced by `compress_simple`.
pub fn decompress_simple(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if i >= data.len() {
            break;
        }
        let marker = data[i];
        i += 1;

        match marker {
            0x00 => {
                // Run: count(u16 LE) + byte
                if i + 3 > data.len() {
                    return Err(ProtocolError::DecompressFailed);
                }
                let count = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
                let byte = data[i + 2];
                i += 3;
                out.resize(out.len() + count, byte);
            }
            0x01 => {
                // Literal: count(u16 LE) + bytes
                if i + 2 > data.len() {
                    return Err(ProtocolError::DecompressFailed);
                }
                let count = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
                i += 2;
                if i + count > data.len() {
                    return Err(ProtocolError::DecompressFailed);
                }
                out.extend_from_slice(&data[i..i + count]);
                i += count;
            }
            _ => {
                return Err(ProtocolError::DecompressFailed);
            }
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- MessageType tests ----------------------------------------------------

    #[test]
    fn test_message_type_roundtrip() {
        let types = [
            MessageType::Connect,
            MessageType::Disconnect,
            MessageType::Ping,
            MessageType::Pong,
            MessageType::StateUpdate,
            MessageType::Input,
            MessageType::Custom,
        ];
        for &mt in &types {
            let byte = mt as u8;
            let decoded = MessageType::from_u8(byte).unwrap();
            assert_eq!(decoded, mt);
        }
    }

    #[test]
    fn test_message_type_invalid() {
        assert_eq!(MessageType::from_u8(7), None);
        assert_eq!(MessageType::from_u8(255), None);
    }

    // -- Header tests ---------------------------------------------------------

    #[test]
    fn test_header_encode_decode_roundtrip() {
        let header = MessageHeader {
            message_type: MessageType::StateUpdate,
            sequence: 42,
            payload_len: 1024,
        };
        let mut buf = [0u8; HEADER_SIZE];
        header.encode(&mut buf);
        let decoded = MessageHeader::decode(&buf).unwrap();
        assert_eq!(decoded, header);
    }

    #[test]
    fn test_header_decode_too_small() {
        let buf = [0u8; 3];
        assert_eq!(MessageHeader::decode(&buf), Err(ProtocolError::BufferTooSmall));
    }

    #[test]
    fn test_header_decode_invalid_type() {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = 99; // invalid type
        assert_eq!(
            MessageHeader::decode(&buf),
            Err(ProtocolError::InvalidMessageType(99))
        );
    }

    // -- Message encode/decode roundtrip for all types ------------------------

    #[test]
    fn test_connect_message_roundtrip() {
        let msg = Message::connect(1, "player1");
        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.header.message_type, MessageType::Connect);
        assert_eq!(decoded.header.sequence, 1);
        assert_eq!(decoded.payload, b"player1");
    }

    #[test]
    fn test_disconnect_message_roundtrip() {
        let msg = Message::disconnect(99);
        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.header.message_type, MessageType::Disconnect);
        assert_eq!(decoded.header.sequence, 99);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn test_ping_pong_roundtrip() {
        let timestamp: u64 = 1234567890;
        let ping = Message::ping(10, timestamp);
        let encoded = encode_message(&ping).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();
        assert_eq!(decoded.header.message_type, MessageType::Ping);
        let ts = u64::from_le_bytes(decoded.payload[..8].try_into().unwrap());
        assert_eq!(ts, timestamp);

        let pong = Message::pong(11, timestamp);
        let encoded = encode_message(&pong).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();
        assert_eq!(decoded.header.message_type, MessageType::Pong);
        let ts = u64::from_le_bytes(decoded.payload[..8].try_into().unwrap());
        assert_eq!(ts, timestamp);
    }

    #[test]
    fn test_state_update_roundtrip() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let msg = Message::state_update(100, data.clone());
        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.header.message_type, MessageType::StateUpdate);
        assert_eq!(decoded.payload, data);
    }

    #[test]
    fn test_input_message_roundtrip() {
        let data = vec![0xAA, 0xBB, 0xCC];
        let msg = Message::input(50, data.clone());
        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();
        assert_eq!(decoded.header.message_type, MessageType::Input);
        assert_eq!(decoded.payload, data);
    }

    #[test]
    fn test_custom_message_roundtrip() {
        let data = b"custom data payload".to_vec();
        let msg = Message::custom(7, data.clone());
        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();
        assert_eq!(decoded.header.message_type, MessageType::Custom);
        assert_eq!(decoded.payload, data);
    }

    #[test]
    fn test_empty_payload_roundtrip() {
        let msg = Message::new(MessageType::Custom, 0, Vec::new());
        let encoded = encode_message(&msg).unwrap();
        assert_eq!(encoded.len(), HEADER_SIZE);
        let (decoded, consumed) = decode_message(&encoded).unwrap();
        assert_eq!(consumed, HEADER_SIZE);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn test_decode_truncated_payload() {
        let msg = Message::new(MessageType::Input, 1, vec![1, 2, 3, 4, 5]);
        let encoded = encode_message(&msg).unwrap();
        // Truncate the payload
        let truncated = &encoded[..HEADER_SIZE + 2];
        let result = decode_message(truncated);
        assert!(matches!(result, Err(ProtocolError::PayloadMismatch { .. })));
    }

    #[test]
    fn test_sequence_number_wrap() {
        let msg = Message::new(MessageType::Ping, u32::MAX, vec![]);
        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();
        assert_eq!(decoded.header.sequence, u32::MAX);
    }

    // -- Message bit-exactness for 10K roundtrips -----------------------------

    #[test]
    fn test_10k_roundtrip_no_corruption() {
        for i in 0..10_000u32 {
            let msg_type = match i % 7 {
                0 => MessageType::Connect,
                1 => MessageType::Disconnect,
                2 => MessageType::Ping,
                3 => MessageType::Pong,
                4 => MessageType::StateUpdate,
                5 => MessageType::Input,
                _ => MessageType::Custom,
            };
            // Deterministic payload based on index
            let payload_len = (i % 128) as usize;
            let payload: Vec<u8> = (0..payload_len).map(|j| ((i + j as u32) & 0xFF) as u8).collect();
            let msg = Message::new(msg_type, i, payload.clone());

            let encoded = encode_message(&msg).unwrap();
            let (decoded, consumed) = decode_message(&encoded).unwrap();

            assert_eq!(consumed, encoded.len());
            assert_eq!(decoded.header.message_type, msg_type);
            assert_eq!(decoded.header.sequence, i);
            assert_eq!(decoded.payload, payload, "corruption at message {i}");
        }
    }

    // -- Compression tests ----------------------------------------------------

    #[test]
    fn test_compress_empty() {
        let compressed = compress_simple(&[]);
        assert!(compressed.is_empty());
        let decompressed = decompress_simple(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_compress_roundtrip_literals() {
        let data = b"hello world! this is a test.";
        let compressed = compress_simple(data);
        let decompressed = decompress_simple(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_roundtrip_runs() {
        // Data with long runs of identical bytes
        let mut data = Vec::new();
        data.extend_from_slice(&[0u8; 100]);
        data.extend_from_slice(&[0xFF; 50]);
        data.extend_from_slice(&[0x42; 200]);

        let compressed = compress_simple(&data);
        let decompressed = decompress_simple(&compressed).unwrap();
        assert_eq!(decompressed, data);

        // Run-heavy data should compress well
        assert!(compressed.len() < data.len() / 2);
    }

    #[test]
    fn test_compress_roundtrip_mixed() {
        // Mix of runs and literals
        let mut data = Vec::new();
        data.extend_from_slice(b"header");
        data.extend_from_slice(&[0u8; 100]);
        data.extend_from_slice(b"middle");
        data.extend_from_slice(&[0xAA; 50]);
        data.extend_from_slice(b"tail");

        let compressed = compress_simple(&data);
        let decompressed = decompress_simple(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_1kb_state_roundtrip() {
        // Simulate a typical state update: mostly zeros with some data
        let mut data = vec![0u8; 1024];
        // Sprinkle some non-zero data (entity positions, etc.)
        for i in (0..1024).step_by(64) {
            data[i] = 0x42;
            if i + 1 < 1024 {
                data[i + 1] = 0x13;
            }
            if i + 2 < 1024 {
                data[i + 2] = 0x37;
            }
        }

        let compressed = compress_simple(&data);
        let decompressed = decompress_simple(&compressed).unwrap();
        assert_eq!(decompressed, data);

        // Should achieve meaningful compression for sparse state data
        assert!(
            compressed.len() < data.len(),
            "compressed {} should be < original {}",
            compressed.len(),
            data.len()
        );
    }

    #[test]
    fn test_decompress_invalid_marker() {
        let data = [0xFF, 0x00, 0x01]; // invalid marker
        assert_eq!(decompress_simple(&data), Err(ProtocolError::DecompressFailed));
    }

    #[test]
    fn test_decompress_truncated_run() {
        // Run marker but not enough bytes for count + value
        let data = [0x00, 0x05];
        assert_eq!(decompress_simple(&data), Err(ProtocolError::DecompressFailed));
    }

    #[test]
    fn test_decompress_truncated_literal() {
        // Literal marker with count but not enough data
        let data = [0x01, 0x0A, 0x00, 0x01]; // says 10 bytes but only 1 available
        assert_eq!(decompress_simple(&data), Err(ProtocolError::DecompressFailed));
    }

    // -- Benchmark: encode/decode throughput ----------------------------------

    #[test]
    fn bench_protocol_throughput() {
        // Encode/decode 100K messages and verify throughput.
        let payload = vec![0x42u8; 64];
        let msg = Message::new(MessageType::StateUpdate, 0, payload);

        let start = std::time::Instant::now();
        let count = 100_000u32;

        for seq in 0..count {
            let mut m = msg.clone();
            m.header.sequence = seq;
            let encoded = encode_message(&m).unwrap();
            let (decoded, _) = decode_message(&encoded).unwrap();
            assert_eq!(decoded.header.sequence, seq);
        }

        let elapsed = start.elapsed();
        let msgs_per_sec = count as f64 / elapsed.as_secs_f64();

        assert!(
            msgs_per_sec >= 100_000.0,
            "Protocol throughput {msgs_per_sec:.0} msg/s < 100K msg/s threshold"
        );
    }
}
