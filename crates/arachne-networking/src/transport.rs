//! Transport layer for network communication.
//!
//! Provides a trait for sending and receiving raw bytes, with concrete
//! implementations for WebSocket (native + WASM stubs) and a mock transport
//! for testing.

/// Errors from transport operations.
#[derive(Clone, Debug, PartialEq)]
pub enum TransportError {
    /// Not currently connected.
    NotConnected,
    /// Connection attempt failed.
    ConnectionFailed(String),
    /// Send operation failed.
    SendFailed(String),
    /// The connection was closed.
    Closed,
    /// The URL or address is invalid.
    InvalidAddress(String),
    /// Platform not supported for this transport.
    Unsupported,
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TransportError::NotConnected => write!(f, "not connected"),
            TransportError::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            TransportError::SendFailed(msg) => write!(f, "send failed: {msg}"),
            TransportError::Closed => write!(f, "connection closed"),
            TransportError::InvalidAddress(msg) => write!(f, "invalid address: {msg}"),
            TransportError::Unsupported => write!(f, "transport not supported on this platform"),
        }
    }
}

/// Trait for a bidirectional byte transport.
///
/// Implementations provide poll-based send/receive suitable for both
/// native and WASM targets (no async runtime required).
pub trait Transport {
    /// Connects to the given URL or address.
    fn connect(&mut self, url: &str) -> Result<(), TransportError>;

    /// Sends raw bytes over the transport.
    fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Polls for received data. Returns `None` if no data is available.
    fn receive(&mut self) -> Result<Option<Vec<u8>>, TransportError>;

    /// Disconnects the transport.
    fn disconnect(&mut self) -> Result<(), TransportError>;

    /// Returns whether the transport is currently connected.
    fn is_connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// WebSocket Transport
// ---------------------------------------------------------------------------

/// Connection state for the WebSocket transport.
#[derive(Clone, Copy, Debug, PartialEq)]
enum WsState {
    Disconnected,
    Connected,
}

/// WebSocket transport implementation.
///
/// On native targets, this would use tungstenite or a similar lightweight
/// WebSocket library. On WASM targets, it uses the web-sys WebSocket API.
/// For now, both provide the interface with stub I/O -- actual network I/O
/// requires platform-specific setup at runtime.
pub struct WebSocketTransport {
    state: WsState,
    url: Option<String>,
    /// Outbound message queue (used in stub mode).
    outbound: Vec<Vec<u8>>,
    /// Inbound message queue (used in stub mode / testing).
    inbound: Vec<Vec<u8>>,
    /// Auto-reconnect: whether to attempt reconnection on disconnect.
    pub auto_reconnect: bool,
    /// Number of reconnection attempts made.
    pub reconnect_attempts: u32,
    /// Maximum reconnection attempts before giving up (0 = unlimited).
    pub max_reconnect_attempts: u32,
}

impl WebSocketTransport {
    /// Creates a new WebSocket transport.
    pub fn new() -> Self {
        Self {
            state: WsState::Disconnected,
            url: None,
            outbound: Vec::new(),
            inbound: Vec::new(),
            auto_reconnect: false,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
        }
    }

    /// Returns the URL this transport is connected to, if any.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Pushes data into the inbound queue (for testing / loopback).
    pub fn inject_inbound(&mut self, data: Vec<u8>) {
        self.inbound.push(data);
    }

    /// Returns the number of outbound messages queued.
    pub fn outbound_count(&self) -> usize {
        self.outbound.len()
    }

    /// Drains all outbound messages (for testing / bridging).
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        core::mem::take(&mut self.outbound)
    }

    /// Calculates exponential backoff delay in milliseconds for reconnection.
    pub fn backoff_ms(&self) -> u64 {
        let base: u64 = 100;
        let max_delay: u64 = 5000;
        let delay = base.saturating_mul(1u64 << self.reconnect_attempts.min(10));
        delay.min(max_delay)
    }
}

impl Default for WebSocketTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Transport for WebSocketTransport {
    fn connect(&mut self, url: &str) -> Result<(), TransportError> {
        if url.is_empty() {
            return Err(TransportError::InvalidAddress("empty URL".into()));
        }
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err(TransportError::InvalidAddress(
                format!("expected ws:// or wss:// scheme, got: {url}"),
            ));
        }

        // NOTE: In a real native implementation, this would:
        // 1. Use tungstenite to establish a WebSocket connection
        // 2. Spawn a background thread / polling task for I/O
        // For now, we transition state to Connected (stub mode).

        self.url = Some(url.to_string());
        self.state = WsState::Connected;
        self.reconnect_attempts = 0;
        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.state != WsState::Connected {
            return Err(TransportError::NotConnected);
        }

        // NOTE: Real implementation would write to the tungstenite socket.
        self.outbound.push(data.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        if self.state != WsState::Connected {
            return Err(TransportError::NotConnected);
        }

        // NOTE: Real implementation would poll the tungstenite socket.
        if self.inbound.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.inbound.remove(0)))
        }
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        if self.state == WsState::Disconnected {
            return Err(TransportError::NotConnected);
        }

        // NOTE: Real implementation would close the tungstenite socket.
        self.state = WsState::Disconnected;
        self.outbound.clear();
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.state == WsState::Connected
    }
}

#[cfg(target_arch = "wasm32")]
impl Transport for WebSocketTransport {
    fn connect(&mut self, url: &str) -> Result<(), TransportError> {
        if url.is_empty() {
            return Err(TransportError::InvalidAddress("empty URL".into()));
        }
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err(TransportError::InvalidAddress(
                format!("expected ws:// or wss:// scheme, got: {url}"),
            ));
        }

        // NOTE: In a real WASM build, this would:
        // 1. Create a web_sys::WebSocket with the URL
        // 2. Set up onopen, onmessage, onerror, onclose callbacks
        // 3. Transition to Connected when onopen fires

        self.url = Some(url.to_string());
        self.state = WsState::Connected;
        self.reconnect_attempts = 0;
        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.state != WsState::Connected {
            return Err(TransportError::NotConnected);
        }

        // NOTE: Real WASM implementation would call WebSocket.send()
        self.outbound.push(data.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        if self.state != WsState::Connected {
            return Err(TransportError::NotConnected);
        }

        // NOTE: Real WASM implementation would drain from onmessage queue
        if self.inbound.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.inbound.remove(0)))
        }
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        if self.state == WsState::Disconnected {
            return Err(TransportError::NotConnected);
        }

        // NOTE: Real WASM implementation would call WebSocket.close()
        self.state = WsState::Disconnected;
        self.outbound.clear();
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.state == WsState::Connected
    }
}

// ---------------------------------------------------------------------------
// Mock Transport (for testing)
// ---------------------------------------------------------------------------

/// A mock transport that operates purely in-memory.
///
/// Useful for unit testing network code without actual sockets.
/// Supports pairing two MockTransports together via manual message passing.
pub struct MockTransport {
    connected: bool,
    url: Option<String>,
    inbound: Vec<Vec<u8>>,
    outbound: Vec<Vec<u8>>,
    /// If true, the next send will fail.
    pub fail_next_send: bool,
    /// If true, the next connect will fail.
    pub fail_next_connect: bool,
}

impl MockTransport {
    /// Creates a new mock transport.
    pub fn new() -> Self {
        Self {
            connected: false,
            url: None,
            inbound: Vec::new(),
            outbound: Vec::new(),
            fail_next_send: false,
            fail_next_connect: false,
        }
    }

    /// Pushes data into the inbound queue, simulating a received message.
    pub fn inject_inbound(&mut self, data: Vec<u8>) {
        self.inbound.push(data);
    }

    /// Returns the number of outbound messages.
    pub fn outbound_count(&self) -> usize {
        self.outbound.len()
    }

    /// Drains all outbound messages.
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        core::mem::take(&mut self.outbound)
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockTransport {
    fn connect(&mut self, url: &str) -> Result<(), TransportError> {
        if self.fail_next_connect {
            self.fail_next_connect = false;
            return Err(TransportError::ConnectionFailed("mock failure".into()));
        }
        self.url = Some(url.to_string());
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        if self.fail_next_send {
            self.fail_next_send = false;
            return Err(TransportError::SendFailed("mock failure".into()));
        }
        self.outbound.push(data.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        if self.inbound.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.inbound.remove(0)))
        }
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        self.connected = false;
        self.outbound.clear();
        self.inbound.clear();
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- MockTransport tests --------------------------------------------------

    #[test]
    fn test_mock_transport_connect_disconnect() {
        let mut t = MockTransport::new();
        assert!(!t.is_connected());

        t.connect("ws://localhost:1234").unwrap();
        assert!(t.is_connected());

        t.disconnect().unwrap();
        assert!(!t.is_connected());
    }

    #[test]
    fn test_mock_transport_send_receive() {
        let mut t = MockTransport::new();
        t.connect("ws://test").unwrap();

        t.send(b"hello").unwrap();
        assert_eq!(t.outbound_count(), 1);

        let msgs = t.drain_outbound();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], b"hello");

        // Inject an inbound message
        t.inject_inbound(b"world".to_vec());
        let received = t.receive().unwrap();
        assert_eq!(received, Some(b"world".to_vec()));

        // No more messages
        let received = t.receive().unwrap();
        assert_eq!(received, None);
    }

    #[test]
    fn test_mock_transport_send_before_connect() {
        let mut t = MockTransport::new();
        assert_eq!(t.send(b"data"), Err(TransportError::NotConnected));
    }

    #[test]
    fn test_mock_transport_receive_before_connect() {
        let mut t = MockTransport::new();
        assert_eq!(t.receive(), Err(TransportError::NotConnected));
    }

    #[test]
    fn test_mock_transport_disconnect_when_not_connected() {
        let mut t = MockTransport::new();
        assert_eq!(t.disconnect(), Err(TransportError::NotConnected));
    }

    #[test]
    fn test_mock_transport_fail_connect() {
        let mut t = MockTransport::new();
        t.fail_next_connect = true;
        let result = t.connect("ws://test");
        assert!(result.is_err());
        assert!(!t.is_connected());
    }

    #[test]
    fn test_mock_transport_fail_send() {
        let mut t = MockTransport::new();
        t.connect("ws://test").unwrap();
        t.fail_next_send = true;
        let result = t.send(b"data");
        assert!(result.is_err());
        // Subsequent sends should work
        t.send(b"ok").unwrap();
        assert_eq!(t.outbound_count(), 1);
    }

    // -- WebSocketTransport tests ---------------------------------------------

    #[test]
    fn test_ws_transport_connect_disconnect() {
        let mut ws = WebSocketTransport::new();
        assert!(!ws.is_connected());

        ws.connect("ws://localhost:8080").unwrap();
        assert!(ws.is_connected());
        assert_eq!(ws.url(), Some("ws://localhost:8080"));

        ws.disconnect().unwrap();
        assert!(!ws.is_connected());
    }

    #[test]
    fn test_ws_transport_invalid_url_empty() {
        let mut ws = WebSocketTransport::new();
        let result = ws.connect("");
        assert!(matches!(result, Err(TransportError::InvalidAddress(_))));
    }

    #[test]
    fn test_ws_transport_invalid_url_scheme() {
        let mut ws = WebSocketTransport::new();
        let result = ws.connect("http://not-websocket");
        assert!(matches!(result, Err(TransportError::InvalidAddress(_))));
    }

    #[test]
    fn test_ws_transport_send_receive_stub() {
        let mut ws = WebSocketTransport::new();
        ws.connect("ws://localhost:9999").unwrap();

        ws.send(b"request").unwrap();
        assert_eq!(ws.outbound_count(), 1);

        ws.inject_inbound(b"response".to_vec());
        let msg = ws.receive().unwrap();
        assert_eq!(msg, Some(b"response".to_vec()));
    }

    #[test]
    fn test_ws_transport_send_while_disconnected() {
        let mut ws = WebSocketTransport::new();
        assert_eq!(ws.send(b"data"), Err(TransportError::NotConnected));
    }

    #[test]
    fn test_ws_transport_receive_while_disconnected() {
        let mut ws = WebSocketTransport::new();
        assert_eq!(ws.receive(), Err(TransportError::NotConnected));
    }

    #[test]
    fn test_ws_transport_wss_scheme() {
        let mut ws = WebSocketTransport::new();
        ws.connect("wss://secure.example.com").unwrap();
        assert!(ws.is_connected());
    }

    #[test]
    fn test_ws_transport_backoff() {
        let mut ws = WebSocketTransport::new();
        ws.reconnect_attempts = 0;
        assert_eq!(ws.backoff_ms(), 100);
        ws.reconnect_attempts = 1;
        assert_eq!(ws.backoff_ms(), 200);
        ws.reconnect_attempts = 2;
        assert_eq!(ws.backoff_ms(), 400);
        ws.reconnect_attempts = 10;
        assert_eq!(ws.backoff_ms(), 5000); // capped
        ws.reconnect_attempts = 20;
        assert_eq!(ws.backoff_ms(), 5000); // still capped
    }

    #[test]
    fn test_ws_transport_drain_outbound() {
        let mut ws = WebSocketTransport::new();
        ws.connect("ws://localhost:1234").unwrap();
        ws.send(b"a").unwrap();
        ws.send(b"b").unwrap();
        ws.send(b"c").unwrap();

        let drained = ws.drain_outbound();
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0], b"a");
        assert_eq!(drained[1], b"b");
        assert_eq!(drained[2], b"c");
        assert_eq!(ws.outbound_count(), 0);
    }

    #[test]
    fn test_ws_transport_multiple_inbound() {
        let mut ws = WebSocketTransport::new();
        ws.connect("ws://localhost:1234").unwrap();

        ws.inject_inbound(b"msg1".to_vec());
        ws.inject_inbound(b"msg2".to_vec());
        ws.inject_inbound(b"msg3".to_vec());

        assert_eq!(ws.receive().unwrap(), Some(b"msg1".to_vec()));
        assert_eq!(ws.receive().unwrap(), Some(b"msg2".to_vec()));
        assert_eq!(ws.receive().unwrap(), Some(b"msg3".to_vec()));
        assert_eq!(ws.receive().unwrap(), None);
    }
}
