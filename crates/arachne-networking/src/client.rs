//! Network client: connects to a server, sends inputs, receives state updates.
//!
//! Implements a connection state machine, ping/latency tracking, and input
//! buffering for client-side prediction.

use crate::protocol::{self, Message, MessageType};
use crate::transport::{Transport, TransportError, MockTransport};

/// Connection state machine states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected to any server.
    Disconnected,
    /// Attempting to establish a connection.
    Connecting,
    /// Connected and communicating with the server.
    Connected,
}

/// Configuration for the network client.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Server URL to connect to.
    pub server_url: String,
    /// Client display name.
    pub client_name: String,
    /// How many input frames to buffer for prediction.
    pub input_buffer_size: usize,
    /// Interval between ping messages (in ticks).
    pub ping_interval_ticks: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            client_name: String::from("Player"),
            input_buffer_size: 16,
            ping_interval_ticks: 60,
        }
    }
}

/// A buffered input frame for client-side prediction.
#[derive(Clone, Debug)]
pub struct InputFrame {
    /// The sequence number this input was sent with.
    pub sequence: u32,
    /// The raw input data.
    pub data: Vec<u8>,
}

/// Network client for connecting to a server.
///
/// Manages the connection lifecycle, sends inputs, receives state updates,
/// and tracks latency via ping/pong.
pub struct NetworkClient {
    config: ClientConfig,
    state: ConnectionState,
    transport: MockTransport,
    /// Monotonically increasing sequence number.
    sequence: u32,
    /// Input buffer for client-side prediction.
    input_buffer: Vec<InputFrame>,
    /// Last measured round-trip time in milliseconds.
    pub rtt_ms: f64,
    /// Smoothed round-trip time (exponential moving average).
    pub smoothed_rtt_ms: f64,
    /// Timestamp of the last sent ping (ms).
    last_ping_time_ms: u64,
    /// Number of ticks since last ping.
    ticks_since_ping: u32,
    /// Received messages waiting to be processed.
    received: Vec<Message>,
}

impl NetworkClient {
    /// Creates a new network client with the given configuration.
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            state: ConnectionState::Disconnected,
            transport: MockTransport::new(),
            sequence: 0,
            input_buffer: Vec::new(),
            rtt_ms: 0.0,
            smoothed_rtt_ms: 0.0,
            last_ping_time_ms: 0,
            ticks_since_ping: 0,
            received: Vec::new(),
        }
    }

    /// Returns the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Returns a reference to the client configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Returns the current sequence number.
    pub fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Returns the number of buffered input frames.
    pub fn input_buffer_len(&self) -> usize {
        self.input_buffer.len()
    }

    /// Initiates a connection to the server.
    ///
    /// Transitions from Disconnected -> Connecting, then sends a Connect message.
    pub fn connect(&mut self) -> Result<(), TransportError> {
        if self.state != ConnectionState::Disconnected {
            return Ok(()); // already connecting or connected
        }

        self.state = ConnectionState::Connecting;
        self.transport.connect(&self.config.server_url)?;

        // Send connect message
        let msg = Message::connect(self.next_sequence(), &self.config.client_name);
        self.send_message(&msg)?;

        Ok(())
    }

    /// Transitions to the Connected state.
    ///
    /// Called when the server acknowledges our connection (e.g., we receive
    /// a Connect response or the first StateUpdate).
    pub fn on_connected(&mut self) {
        if self.state == ConnectionState::Connecting {
            self.state = ConnectionState::Connected;
        }
    }

    /// Disconnects from the server.
    pub fn disconnect(&mut self) -> Result<(), TransportError> {
        if self.state == ConnectionState::Disconnected {
            return Ok(());
        }

        // Send disconnect message (best-effort)
        let msg = Message::disconnect(self.next_sequence());
        let _ = self.send_message(&msg);

        self.transport.disconnect()?;
        self.state = ConnectionState::Disconnected;
        self.input_buffer.clear();
        self.received.clear();
        Ok(())
    }

    /// Sends an input frame to the server and buffers it for prediction.
    pub fn send_input(&mut self, data: Vec<u8>) -> Result<(), TransportError> {
        if self.state != ConnectionState::Connected {
            return Err(TransportError::NotConnected);
        }

        let seq = self.next_sequence();
        let msg = Message::input(seq, data.clone());
        self.send_message(&msg)?;

        // Buffer for prediction
        self.input_buffer.push(InputFrame {
            sequence: seq,
            data,
        });

        // Trim buffer to configured size
        while self.input_buffer.len() > self.config.input_buffer_size {
            self.input_buffer.remove(0);
        }

        Ok(())
    }

    /// Sends a ping message for latency measurement.
    pub fn send_ping(&mut self, current_time_ms: u64) -> Result<(), TransportError> {
        if self.state != ConnectionState::Connected {
            return Err(TransportError::NotConnected);
        }

        let msg = Message::ping(self.next_sequence(), current_time_ms);
        self.send_message(&msg)?;
        self.last_ping_time_ms = current_time_ms;
        self.ticks_since_ping = 0;
        Ok(())
    }

    /// Handles a received Pong message, updating RTT.
    pub fn handle_pong(&mut self, current_time_ms: u64, pong_payload: &[u8]) {
        if pong_payload.len() >= 8 {
            let ping_time = u64::from_le_bytes(pong_payload[..8].try_into().unwrap());
            let rtt = current_time_ms.saturating_sub(ping_time) as f64;
            self.rtt_ms = rtt;
            // Exponential moving average (alpha = 0.2)
            if self.smoothed_rtt_ms == 0.0 {
                self.smoothed_rtt_ms = rtt;
            } else {
                self.smoothed_rtt_ms = self.smoothed_rtt_ms * 0.8 + rtt * 0.2;
            }
        }
    }

    /// Polls the transport for incoming messages.
    ///
    /// Decodes received bytes and queues them for processing.
    pub fn poll(&mut self) -> Result<(), TransportError> {
        loop {
            match self.transport.receive()? {
                Some(data) => {
                    if let Ok((msg, _)) = protocol::decode_message(&data) {
                        self.received.push(msg);
                    }
                }
                None => break,
            }
        }
        Ok(())
    }

    /// Drains all received messages.
    pub fn drain_received(&mut self) -> Vec<Message> {
        core::mem::take(&mut self.received)
    }

    /// Acknowledges a server state update, removing confirmed inputs from the buffer.
    ///
    /// The server includes the last processed input sequence number in its
    /// state updates. All inputs up to and including that sequence are confirmed.
    pub fn acknowledge_inputs(&mut self, confirmed_sequence: u32) {
        self.input_buffer.retain(|frame| frame.sequence > confirmed_sequence);
    }

    /// Called once per tick. Handles periodic tasks like pinging.
    pub fn tick(&mut self, current_time_ms: u64) {
        self.ticks_since_ping += 1;
        if self.state == ConnectionState::Connected
            && self.ticks_since_ping >= self.config.ping_interval_ticks
        {
            let _ = self.send_ping(current_time_ms);
        }
    }

    /// Provides mutable access to the underlying transport (for testing).
    pub fn transport_mut(&mut self) -> &mut MockTransport {
        &mut self.transport
    }

    // -- Internal helpers -----------------------------------------------------

    fn next_sequence(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }

    fn send_message(&mut self, msg: &Message) -> Result<(), TransportError> {
        let data = protocol::encode_message(msg)
            .map_err(|_| TransportError::SendFailed("encode failed".into()))?;
        self.transport.send(&data)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> NetworkClient {
        NetworkClient::new(ClientConfig {
            server_url: "ws://localhost:9999".into(),
            client_name: "TestPlayer".into(),
            input_buffer_size: 8,
            ping_interval_ticks: 30,
        })
    }

    // -- Connection state machine tests ---------------------------------------

    #[test]
    fn test_initial_state_is_disconnected() {
        let client = make_client();
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_connect_transitions_to_connecting() {
        let mut client = make_client();
        client.connect().unwrap();
        assert_eq!(client.state(), ConnectionState::Connecting);
    }

    #[test]
    fn test_on_connected_transitions_to_connected() {
        let mut client = make_client();
        client.connect().unwrap();
        assert_eq!(client.state(), ConnectionState::Connecting);
        client.on_connected();
        assert_eq!(client.state(), ConnectionState::Connected);
    }

    #[test]
    fn test_disconnect_transitions_to_disconnected() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();
        client.disconnect().unwrap();
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_disconnect_from_connecting() {
        let mut client = make_client();
        client.connect().unwrap();
        assert_eq!(client.state(), ConnectionState::Connecting);
        client.disconnect().unwrap();
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_on_connected_only_from_connecting() {
        let mut client = make_client();
        // on_connected from Disconnected should be a no-op
        client.on_connected();
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_connect_sends_connect_message() {
        let mut client = make_client();
        client.connect().unwrap();
        // The transport should have an outbound connect message
        let outbound = client.transport_mut().drain_outbound();
        assert_eq!(outbound.len(), 1);
        let (msg, _) = protocol::decode_message(&outbound[0]).unwrap();
        assert_eq!(msg.header.message_type, MessageType::Connect);
        assert_eq!(msg.payload, b"TestPlayer");
    }

    // -- Input buffering tests ------------------------------------------------

    #[test]
    fn test_send_input_requires_connected() {
        let mut client = make_client();
        assert_eq!(
            client.send_input(vec![1, 2, 3]),
            Err(TransportError::NotConnected)
        );
    }

    #[test]
    fn test_send_input_buffers_and_sends() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();
        client.transport_mut().drain_outbound(); // clear connect msg

        client.send_input(vec![10, 20]).unwrap();
        client.send_input(vec![30, 40]).unwrap();

        assert_eq!(client.input_buffer_len(), 2);

        let outbound = client.transport_mut().drain_outbound();
        assert_eq!(outbound.len(), 2);
    }

    #[test]
    fn test_input_buffer_trimming() {
        let mut client = NetworkClient::new(ClientConfig {
            server_url: "ws://localhost:9999".into(),
            client_name: "Test".into(),
            input_buffer_size: 3,
            ping_interval_ticks: 60,
        });
        client.connect().unwrap();
        client.on_connected();

        for i in 0..5 {
            client.send_input(vec![i]).unwrap();
        }

        // Buffer should be trimmed to 3
        assert_eq!(client.input_buffer_len(), 3);
    }

    #[test]
    fn test_acknowledge_inputs() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();

        // Send some inputs (sequences start at 1, since 0 was the connect msg)
        client.send_input(vec![1]).unwrap(); // seq 1
        client.send_input(vec![2]).unwrap(); // seq 2
        client.send_input(vec![3]).unwrap(); // seq 3

        assert_eq!(client.input_buffer_len(), 3);

        // Acknowledge up to sequence 2
        client.acknowledge_inputs(2);
        assert_eq!(client.input_buffer_len(), 1); // only seq 3 remains
    }

    // -- Ping/latency tests ---------------------------------------------------

    #[test]
    fn test_send_ping() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();
        client.transport_mut().drain_outbound();

        client.send_ping(1000).unwrap();

        let outbound = client.transport_mut().drain_outbound();
        assert_eq!(outbound.len(), 1);
        let (msg, _) = protocol::decode_message(&outbound[0]).unwrap();
        assert_eq!(msg.header.message_type, MessageType::Ping);
    }

    #[test]
    fn test_handle_pong_updates_rtt() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();

        // Simulate ping at t=1000, pong at t=1050
        let ping_time: u64 = 1000;
        client.handle_pong(1050, &ping_time.to_le_bytes());

        assert!((client.rtt_ms - 50.0).abs() < 0.001);
        assert!((client.smoothed_rtt_ms - 50.0).abs() < 0.001); // first sample
    }

    #[test]
    fn test_smoothed_rtt_averaging() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();

        // First sample: RTT = 100
        client.handle_pong(1100, &1000u64.to_le_bytes());
        assert!((client.smoothed_rtt_ms - 100.0).abs() < 0.001);

        // Second sample: RTT = 50
        client.handle_pong(2050, &2000u64.to_le_bytes());
        // smoothed = 100 * 0.8 + 50 * 0.2 = 90
        assert!((client.smoothed_rtt_ms - 90.0).abs() < 0.001);
    }

    // -- Poll / message receive tests -----------------------------------------

    #[test]
    fn test_poll_and_drain() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();

        // Inject a state update message into the transport
        let msg = Message::state_update(1, vec![0xDE, 0xAD]);
        let encoded = protocol::encode_message(&msg).unwrap();
        client.transport_mut().inject_inbound(encoded);

        client.poll().unwrap();
        let received = client.drain_received();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].header.message_type, MessageType::StateUpdate);
        assert_eq!(received[0].payload, vec![0xDE, 0xAD]);
    }

    #[test]
    fn test_disconnect_clears_buffers() {
        let mut client = make_client();
        client.connect().unwrap();
        client.on_connected();

        client.send_input(vec![1]).unwrap();
        let msg = Message::state_update(1, vec![1]);
        let encoded = protocol::encode_message(&msg).unwrap();
        client.transport_mut().inject_inbound(encoded);
        client.poll().unwrap();

        client.disconnect().unwrap();
        assert_eq!(client.input_buffer_len(), 0);
        assert!(client.drain_received().is_empty());
    }

    // -- Tick auto-ping test --------------------------------------------------

    #[test]
    fn test_tick_auto_ping() {
        let mut client = NetworkClient::new(ClientConfig {
            server_url: "ws://localhost:9999".into(),
            client_name: "Test".into(),
            input_buffer_size: 8,
            ping_interval_ticks: 5,
        });
        client.connect().unwrap();
        client.on_connected();
        client.transport_mut().drain_outbound();

        // Tick 4 times -- no ping yet
        for t in 0..4 {
            client.tick(t * 16);
            assert_eq!(client.transport_mut().outbound_count(), 0);
        }

        // Tick 5th time -- should trigger ping
        client.tick(80);
        assert_eq!(client.transport_mut().outbound_count(), 1);
    }

    #[test]
    fn test_sequence_increments() {
        let mut client = make_client();
        assert_eq!(client.sequence(), 0);
        client.connect().unwrap(); // uses seq 0
        assert_eq!(client.sequence(), 1);
        client.on_connected();
        client.send_input(vec![1]).unwrap(); // uses seq 1
        assert_eq!(client.sequence(), 2);
    }
}
