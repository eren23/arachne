//! Network server: accepts connections, broadcasts state, receives inputs.
//!
//! Implements a server-authoritative model with tick-based updates and
//! client slot management.

use crate::protocol::{self, Message, MessageType};
use crate::transport::{Transport, TransportError, MockTransport};

/// Maximum number of client slots.
pub const MAX_CLIENTS: usize = 32;

/// Unique identifier for a client slot.
pub type SlotId = u8;

/// Configuration for the network server.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Address to listen on.
    pub listen_url: String,
    /// Maximum number of connected clients.
    pub max_clients: usize,
    /// Tick rate (ticks per second).
    pub tick_rate: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_url: String::from("ws://0.0.0.0:9999"),
            max_clients: MAX_CLIENTS,
            tick_rate: 60,
        }
    }
}

/// State of a connected client slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientSlotState {
    /// Slot is available.
    Empty,
    /// Client is connected and active.
    Connected,
}

/// Information about a connected client.
#[derive(Clone, Debug)]
pub struct ClientSlot {
    pub slot_id: SlotId,
    pub state: ClientSlotState,
    pub name: String,
    /// Last received sequence number from this client.
    pub last_input_sequence: u32,
    /// Buffered inputs from this client.
    pub input_buffer: Vec<Vec<u8>>,
    /// Ping RTT in milliseconds (as measured by server).
    pub rtt_ms: f64,
}

impl ClientSlot {
    fn new(slot_id: SlotId) -> Self {
        Self {
            slot_id,
            state: ClientSlotState::Empty,
            name: String::new(),
            last_input_sequence: 0,
            input_buffer: Vec::new(),
            rtt_ms: 0.0,
        }
    }

    fn reset(&mut self) {
        self.state = ClientSlotState::Empty;
        self.name.clear();
        self.last_input_sequence = 0;
        self.input_buffer.clear();
        self.rtt_ms = 0.0;
    }
}

/// Events emitted by the server during processing.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerEvent {
    /// A new client connected.
    ClientConnected { slot_id: SlotId, name: String },
    /// A client disconnected.
    ClientDisconnected { slot_id: SlotId },
    /// Received input from a client.
    InputReceived { slot_id: SlotId, sequence: u32, data: Vec<u8> },
    /// Server is full, connection rejected.
    ConnectionRejected,
}

/// Network server that manages client connections and state broadcasting.
///
/// Uses a slot-based model where each connected client occupies a numbered
/// slot. The server is authoritative: it processes inputs and broadcasts
/// the canonical game state.
pub struct NetworkServer {
    config: ServerConfig,
    slots: Vec<ClientSlot>,
    /// Per-slot mock transports (for testing; real server would have real sockets).
    transports: Vec<MockTransport>,
    /// Current server tick.
    tick: u64,
    /// Sequence number for server-originated messages.
    sequence: u32,
    /// Events generated during the last process cycle.
    events: Vec<ServerEvent>,
}

impl NetworkServer {
    /// Creates a new network server with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        let max = config.max_clients.min(MAX_CLIENTS);
        let mut slots = Vec::with_capacity(max);
        let mut transports = Vec::with_capacity(max);
        for i in 0..max {
            slots.push(ClientSlot::new(i as SlotId));
            transports.push(MockTransport::new());
        }
        Self {
            config,
            slots,
            transports,
            tick: 0,
            sequence: 0,
            events: Vec::new(),
        }
    }

    /// Returns the current server tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns a reference to the server configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Returns the number of currently connected clients.
    pub fn connected_count(&self) -> usize {
        self.slots.iter().filter(|s| s.state == ClientSlotState::Connected).count()
    }

    /// Returns a reference to a client slot.
    pub fn slot(&self, slot_id: SlotId) -> Option<&ClientSlot> {
        self.slots.get(slot_id as usize)
    }

    /// Returns a mutable reference to a client slot.
    pub fn slot_mut(&mut self, slot_id: SlotId) -> Option<&mut ClientSlot> {
        self.slots.get_mut(slot_id as usize)
    }

    /// Returns all connected slot IDs.
    pub fn connected_slots(&self) -> Vec<SlotId> {
        self.slots
            .iter()
            .filter(|s| s.state == ClientSlotState::Connected)
            .map(|s| s.slot_id)
            .collect()
    }

    /// Accepts a new client connection, assigning it to the first available slot.
    ///
    /// Returns the slot ID on success, or an error if the server is full.
    pub fn accept_client(&mut self, name: &str) -> Result<SlotId, ServerEvent> {
        let slot_idx = self.slots.iter().position(|s| s.state == ClientSlotState::Empty);
        match slot_idx {
            Some(idx) => {
                let slot_id = idx as SlotId;
                self.slots[idx].state = ClientSlotState::Connected;
                self.slots[idx].name = name.to_string();
                // Connect the mock transport for this slot
                let _ = self.transports[idx].connect(&self.config.listen_url);
                self.events.push(ServerEvent::ClientConnected {
                    slot_id,
                    name: name.to_string(),
                });
                Ok(slot_id)
            }
            None => {
                self.events.push(ServerEvent::ConnectionRejected);
                Err(ServerEvent::ConnectionRejected)
            }
        }
    }

    /// Disconnects a client by slot ID.
    pub fn disconnect_client(&mut self, slot_id: SlotId) {
        if let Some(slot) = self.slots.get_mut(slot_id as usize) {
            if slot.state == ClientSlotState::Connected {
                slot.reset();
                let _ = self.transports[slot_id as usize].disconnect();
                self.events.push(ServerEvent::ClientDisconnected { slot_id });
            }
        }
    }

    /// Receives input from a client.
    pub fn receive_input(&mut self, slot_id: SlotId, sequence: u32, data: Vec<u8>) {
        if let Some(slot) = self.slots.get_mut(slot_id as usize) {
            if slot.state == ClientSlotState::Connected {
                slot.last_input_sequence = sequence;
                slot.input_buffer.push(data.clone());
                self.events.push(ServerEvent::InputReceived {
                    slot_id,
                    sequence,
                    data,
                });
            }
        }
    }

    /// Broadcasts a state update to all connected clients.
    pub fn broadcast_state(&mut self, state_data: &[u8]) -> Result<(), TransportError> {
        let seq = self.next_sequence();
        let msg = Message::state_update(seq, state_data.to_vec());
        let encoded = protocol::encode_message(&msg)
            .map_err(|_| TransportError::SendFailed("encode failed".into()))?;

        for (i, slot) in self.slots.iter().enumerate() {
            if slot.state == ClientSlotState::Connected {
                if self.transports[i].is_connected() {
                    self.transports[i].send(&encoded)?;
                }
            }
        }

        Ok(())
    }

    /// Sends a message to a specific client.
    pub fn send_to(&mut self, slot_id: SlotId, msg: &Message) -> Result<(), TransportError> {
        let idx = slot_id as usize;
        if idx >= self.slots.len() || self.slots[idx].state != ClientSlotState::Connected {
            return Err(TransportError::NotConnected);
        }

        let encoded = protocol::encode_message(msg)
            .map_err(|_| TransportError::SendFailed("encode failed".into()))?;
        self.transports[idx].send(&encoded)
    }

    /// Handles a ping from a client by sending back a pong.
    pub fn handle_ping(&mut self, slot_id: SlotId, ping_payload: &[u8]) -> Result<(), TransportError> {
        let seq = self.next_sequence();
        let msg = Message::new(MessageType::Pong, seq, ping_payload.to_vec());
        self.send_to(slot_id, &msg)
    }

    /// Advances the server tick.
    pub fn advance_tick(&mut self) {
        self.tick += 1;
    }

    /// Drains all pending server events.
    pub fn drain_events(&mut self) -> Vec<ServerEvent> {
        core::mem::take(&mut self.events)
    }

    /// Drains inputs from a specific client slot.
    pub fn drain_inputs(&mut self, slot_id: SlotId) -> Vec<Vec<u8>> {
        if let Some(slot) = self.slots.get_mut(slot_id as usize) {
            core::mem::take(&mut slot.input_buffer)
        } else {
            Vec::new()
        }
    }

    /// Returns a mutable reference to a slot's transport (for testing).
    pub fn transport_mut(&mut self, slot_id: SlotId) -> Option<&mut MockTransport> {
        self.transports.get_mut(slot_id as usize)
    }

    // -- Internal helpers -----------------------------------------------------

    fn next_sequence(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server() -> NetworkServer {
        NetworkServer::new(ServerConfig {
            listen_url: "ws://0.0.0.0:9999".into(),
            max_clients: 4,
            tick_rate: 60,
        })
    }

    // -- Client slot management -----------------------------------------------

    #[test]
    fn test_initial_state() {
        let server = make_server();
        assert_eq!(server.connected_count(), 0);
        assert_eq!(server.tick(), 0);
        assert_eq!(server.config().max_clients, 4);
    }

    #[test]
    fn test_accept_client() {
        let mut server = make_server();
        let slot = server.accept_client("Alice").unwrap();
        assert_eq!(slot, 0);
        assert_eq!(server.connected_count(), 1);

        let slot_info = server.slot(0).unwrap();
        assert_eq!(slot_info.state, ClientSlotState::Connected);
        assert_eq!(slot_info.name, "Alice");
    }

    #[test]
    fn test_accept_multiple_clients() {
        let mut server = make_server();
        let s0 = server.accept_client("Alice").unwrap();
        let s1 = server.accept_client("Bob").unwrap();
        let s2 = server.accept_client("Charlie").unwrap();

        assert_eq!(s0, 0);
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert_eq!(server.connected_count(), 3);
    }

    #[test]
    fn test_server_full() {
        let mut server = NetworkServer::new(ServerConfig {
            listen_url: "ws://0.0.0.0:9999".into(),
            max_clients: 2,
            tick_rate: 60,
        });

        server.accept_client("A").unwrap();
        server.accept_client("B").unwrap();
        let result = server.accept_client("C");
        assert!(result.is_err());
    }

    #[test]
    fn test_disconnect_client() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();
        assert_eq!(server.connected_count(), 1);

        server.disconnect_client(0);
        assert_eq!(server.connected_count(), 0);
        assert_eq!(server.slot(0).unwrap().state, ClientSlotState::Empty);
    }

    #[test]
    fn test_slot_reuse_after_disconnect() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap(); // slot 0
        server.accept_client("Bob").unwrap();   // slot 1
        server.disconnect_client(0);            // free slot 0
        let slot = server.accept_client("Charlie").unwrap(); // should reuse slot 0
        assert_eq!(slot, 0);
        assert_eq!(server.slot(0).unwrap().name, "Charlie");
    }

    #[test]
    fn test_connected_slots() {
        let mut server = make_server();
        server.accept_client("A").unwrap();
        server.accept_client("B").unwrap();
        server.accept_client("C").unwrap();
        server.disconnect_client(1);

        let connected = server.connected_slots();
        assert_eq!(connected, vec![0, 2]);
    }

    // -- Input handling -------------------------------------------------------

    #[test]
    fn test_receive_input() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();

        server.receive_input(0, 1, vec![10, 20]);
        server.receive_input(0, 2, vec![30, 40]);

        let inputs = server.drain_inputs(0);
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0], vec![10, 20]);
        assert_eq!(inputs[1], vec![30, 40]);
    }

    #[test]
    fn test_receive_input_updates_sequence() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();

        server.receive_input(0, 5, vec![1]);
        assert_eq!(server.slot(0).unwrap().last_input_sequence, 5);

        server.receive_input(0, 10, vec![2]);
        assert_eq!(server.slot(0).unwrap().last_input_sequence, 10);
    }

    // -- Broadcasting ---------------------------------------------------------

    #[test]
    fn test_broadcast_state() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();
        server.accept_client("Bob").unwrap();

        let state_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        server.broadcast_state(&state_data).unwrap();

        // Both clients should have received the message
        for slot_id in 0..2u8 {
            let transport = server.transport_mut(slot_id).unwrap();
            let outbound = transport.drain_outbound();
            assert_eq!(outbound.len(), 1);
            let (msg, _) = protocol::decode_message(&outbound[0]).unwrap();
            assert_eq!(msg.header.message_type, MessageType::StateUpdate);
            assert_eq!(msg.payload, state_data);
        }
    }

    #[test]
    fn test_broadcast_skips_disconnected() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();
        server.accept_client("Bob").unwrap();
        server.disconnect_client(1);

        server.broadcast_state(&[1, 2, 3]).unwrap();

        // Only slot 0 should have the message
        let t0_out = server.transport_mut(0).unwrap().drain_outbound();
        assert_eq!(t0_out.len(), 1);

        // Slot 1 is disconnected, transport not connected
        let t1_out = server.transport_mut(1).unwrap().outbound_count();
        assert_eq!(t1_out, 0);
    }

    // -- Send to specific client ----------------------------------------------

    #[test]
    fn test_send_to_client() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();

        let msg = Message::pong(1, 42);
        server.send_to(0, &msg).unwrap();

        let outbound = server.transport_mut(0).unwrap().drain_outbound();
        assert_eq!(outbound.len(), 1);
        let (decoded, _) = protocol::decode_message(&outbound[0]).unwrap();
        assert_eq!(decoded.header.message_type, MessageType::Pong);
    }

    #[test]
    fn test_send_to_disconnected_fails() {
        let mut server = make_server();
        let msg = Message::pong(1, 42);
        let result = server.send_to(0, &msg);
        assert!(result.is_err());
    }

    // -- Ping handling --------------------------------------------------------

    #[test]
    fn test_handle_ping() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();

        let ping_payload = 12345u64.to_le_bytes();
        server.handle_ping(0, &ping_payload).unwrap();

        let outbound = server.transport_mut(0).unwrap().drain_outbound();
        assert_eq!(outbound.len(), 1);
        let (msg, _) = protocol::decode_message(&outbound[0]).unwrap();
        assert_eq!(msg.header.message_type, MessageType::Pong);
        assert_eq!(msg.payload, ping_payload);
    }

    // -- Tick and events ------------------------------------------------------

    #[test]
    fn test_advance_tick() {
        let mut server = make_server();
        assert_eq!(server.tick(), 0);
        server.advance_tick();
        assert_eq!(server.tick(), 1);
        server.advance_tick();
        assert_eq!(server.tick(), 2);
    }

    #[test]
    fn test_server_events() {
        let mut server = make_server();
        server.accept_client("Alice").unwrap();
        server.receive_input(0, 1, vec![10]);
        server.disconnect_client(0);

        let events = server.drain_events();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], ServerEvent::ClientConnected { slot_id: 0, .. }));
        assert!(matches!(events[1], ServerEvent::InputReceived { slot_id: 0, sequence: 1, .. }));
        assert!(matches!(events[2], ServerEvent::ClientDisconnected { slot_id: 0 }));

        // Events should be drained
        assert!(server.drain_events().is_empty());
    }

    #[test]
    fn test_server_events_connection_rejected() {
        let mut server = NetworkServer::new(ServerConfig {
            listen_url: "ws://0.0.0.0:9999".into(),
            max_clients: 1,
            tick_rate: 60,
        });

        server.accept_client("A").unwrap();
        let _ = server.accept_client("B"); // rejected

        let events = server.drain_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[1], ServerEvent::ConnectionRejected));
    }
}
