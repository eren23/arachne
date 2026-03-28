//! Simple lobby and matchmaking system.
//!
//! Provides room creation, join/leave, ready state tracking, and game start
//! coordination. Designed for lightweight session-based multiplayer.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

extern crate alloc;

/// Unique identifier for a room.
pub type RoomId = u32;

/// Unique identifier for a player.
pub type PlayerId = u32;

/// Ready state of a player in a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadyState {
    /// Player is not ready.
    NotReady,
    /// Player is ready to start.
    Ready,
}

/// State of a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomState {
    /// Room is in the lobby, waiting for players.
    Waiting,
    /// All players are ready, countdown to start.
    AllReady,
    /// Game has started.
    InGame,
    /// Room is closed.
    Closed,
}

/// Information about a player in a room.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub name: String,
    pub ready: ReadyState,
}

/// A multiplayer room.
#[derive(Clone, Debug)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub max_players: usize,
    pub state: RoomState,
    pub host: PlayerId,
    pub players: Vec<PlayerInfo>,
}

impl Room {
    /// Creates a new room with the given host player.
    fn new(id: RoomId, name: String, max_players: usize, host_id: PlayerId, host_name: String) -> Self {
        let host_info = PlayerInfo {
            id: host_id,
            name: host_name,
            ready: ReadyState::NotReady,
        };
        Self {
            id,
            name,
            max_players,
            state: RoomState::Waiting,
            host: host_id,
            players: vec![host_info],
        }
    }

    /// Returns the number of players in the room.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Returns whether the room is full.
    pub fn is_full(&self) -> bool {
        self.players.len() >= self.max_players
    }

    /// Returns whether all players in the room are ready.
    pub fn all_ready(&self) -> bool {
        !self.players.is_empty()
            && self.players.iter().all(|p| p.ready == ReadyState::Ready)
    }

    /// Finds a player by ID.
    pub fn find_player(&self, player_id: PlayerId) -> Option<&PlayerInfo> {
        self.players.iter().find(|p| p.id == player_id)
    }

    /// Returns a list of player names.
    pub fn player_names(&self) -> Vec<&str> {
        self.players.iter().map(|p| p.name.as_str()).collect()
    }
}

/// Errors from lobby operations.
#[derive(Clone, Debug, PartialEq)]
pub enum LobbyError {
    /// Room not found.
    RoomNotFound(RoomId),
    /// Room is full.
    RoomFull(RoomId),
    /// Player is already in the room.
    AlreadyInRoom(PlayerId),
    /// Player is not in the room.
    NotInRoom(PlayerId),
    /// Room is not in the correct state for this operation.
    InvalidState(RoomState),
    /// Only the host can perform this action.
    NotHost,
}

impl core::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LobbyError::RoomNotFound(id) => write!(f, "room {id} not found"),
            LobbyError::RoomFull(id) => write!(f, "room {id} is full"),
            LobbyError::AlreadyInRoom(id) => write!(f, "player {id} already in room"),
            LobbyError::NotInRoom(id) => write!(f, "player {id} not in room"),
            LobbyError::InvalidState(state) => write!(f, "invalid room state: {state:?}"),
            LobbyError::NotHost => write!(f, "only the host can perform this action"),
        }
    }
}

/// Lobby system managing rooms and matchmaking.
pub struct Lobby {
    rooms: BTreeMap<RoomId, Room>,
    next_room_id: RoomId,
}

impl Lobby {
    /// Creates a new empty lobby.
    pub fn new() -> Self {
        Self {
            rooms: BTreeMap::new(),
            next_room_id: 1,
        }
    }

    /// Returns the number of rooms.
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Returns a reference to a room by ID.
    pub fn room(&self, room_id: RoomId) -> Option<&Room> {
        self.rooms.get(&room_id)
    }

    /// Returns all room IDs.
    pub fn room_ids(&self) -> Vec<RoomId> {
        self.rooms.keys().copied().collect()
    }

    /// Lists all rooms that are in the Waiting state (joinable).
    pub fn list_joinable(&self) -> Vec<&Room> {
        self.rooms
            .values()
            .filter(|r| r.state == RoomState::Waiting && !r.is_full())
            .collect()
    }

    /// Creates a new room. The creating player becomes the host.
    pub fn create_room(
        &mut self,
        room_name: &str,
        max_players: usize,
        host_id: PlayerId,
        host_name: &str,
    ) -> RoomId {
        let id = self.next_room_id;
        self.next_room_id += 1;
        let room = Room::new(
            id,
            room_name.to_string(),
            max_players.max(1),
            host_id,
            host_name.to_string(),
        );
        self.rooms.insert(id, room);
        id
    }

    /// A player joins an existing room.
    pub fn join_room(
        &mut self,
        room_id: RoomId,
        player_id: PlayerId,
        player_name: &str,
    ) -> Result<(), LobbyError> {
        let room = self.rooms.get_mut(&room_id)
            .ok_or(LobbyError::RoomNotFound(room_id))?;

        if room.state != RoomState::Waiting {
            return Err(LobbyError::InvalidState(room.state));
        }
        if room.is_full() {
            return Err(LobbyError::RoomFull(room_id));
        }
        if room.players.iter().any(|p| p.id == player_id) {
            return Err(LobbyError::AlreadyInRoom(player_id));
        }

        room.players.push(PlayerInfo {
            id: player_id,
            name: player_name.to_string(),
            ready: ReadyState::NotReady,
        });

        Ok(())
    }

    /// A player leaves a room.
    ///
    /// If the host leaves, the next player becomes host. If no players remain,
    /// the room is removed.
    pub fn leave_room(
        &mut self,
        room_id: RoomId,
        player_id: PlayerId,
    ) -> Result<(), LobbyError> {
        let room = self.rooms.get_mut(&room_id)
            .ok_or(LobbyError::RoomNotFound(room_id))?;

        let idx = room.players.iter().position(|p| p.id == player_id)
            .ok_or(LobbyError::NotInRoom(player_id))?;

        room.players.remove(idx);

        if room.players.is_empty() {
            // Room is empty, remove it
            self.rooms.remove(&room_id);
        } else if room.host == player_id {
            // Transfer host to next player
            room.host = room.players[0].id;
        }

        Ok(())
    }

    /// Sets a player's ready state.
    pub fn set_ready(
        &mut self,
        room_id: RoomId,
        player_id: PlayerId,
        ready: ReadyState,
    ) -> Result<(), LobbyError> {
        let room = self.rooms.get_mut(&room_id)
            .ok_or(LobbyError::RoomNotFound(room_id))?;

        if room.state != RoomState::Waiting && room.state != RoomState::AllReady {
            return Err(LobbyError::InvalidState(room.state));
        }

        let player = room.players.iter_mut().find(|p| p.id == player_id)
            .ok_or(LobbyError::NotInRoom(player_id))?;

        player.ready = ready;

        // Check if all ready
        if room.all_ready() {
            room.state = RoomState::AllReady;
        } else {
            room.state = RoomState::Waiting;
        }

        Ok(())
    }

    /// Starts the game in a room. Only the host can start, and all players
    /// must be ready.
    pub fn start_game(
        &mut self,
        room_id: RoomId,
        host_id: PlayerId,
    ) -> Result<(), LobbyError> {
        let room = self.rooms.get_mut(&room_id)
            .ok_or(LobbyError::RoomNotFound(room_id))?;

        if room.host != host_id {
            return Err(LobbyError::NotHost);
        }

        if room.state != RoomState::AllReady {
            return Err(LobbyError::InvalidState(room.state));
        }

        room.state = RoomState::InGame;
        Ok(())
    }

    /// Closes a room (e.g., game ended). Only the host can close.
    pub fn close_room(
        &mut self,
        room_id: RoomId,
        host_id: PlayerId,
    ) -> Result<(), LobbyError> {
        let room = self.rooms.get_mut(&room_id)
            .ok_or(LobbyError::RoomNotFound(room_id))?;

        if room.host != host_id {
            return Err(LobbyError::NotHost);
        }

        room.state = RoomState::Closed;
        Ok(())
    }

    /// Removes all closed rooms from the lobby.
    pub fn cleanup_closed(&mut self) {
        self.rooms.retain(|_, r| r.state != RoomState::Closed);
    }
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Room creation --------------------------------------------------------

    #[test]
    fn test_create_room() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test Room", 4, 1, "Alice");

        assert_eq!(lobby.room_count(), 1);
        let room = lobby.room(room_id).unwrap();
        assert_eq!(room.name, "Test Room");
        assert_eq!(room.max_players, 4);
        assert_eq!(room.host, 1);
        assert_eq!(room.player_count(), 1);
        assert_eq!(room.state, RoomState::Waiting);
        assert_eq!(room.players[0].name, "Alice");
    }

    #[test]
    fn test_create_multiple_rooms() {
        let mut lobby = Lobby::new();
        let r1 = lobby.create_room("Room 1", 2, 1, "Alice");
        let r2 = lobby.create_room("Room 2", 4, 2, "Bob");

        assert_eq!(lobby.room_count(), 2);
        assert_ne!(r1, r2);
    }

    // -- Join / Leave ---------------------------------------------------------

    #[test]
    fn test_join_room() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");

        lobby.join_room(room_id, 2, "Bob").unwrap();
        let room = lobby.room(room_id).unwrap();
        assert_eq!(room.player_count(), 2);
        assert_eq!(room.player_names(), vec!["Alice", "Bob"]);
    }

    #[test]
    fn test_join_room_full() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Small", 2, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        let result = lobby.join_room(room_id, 3, "Charlie");
        assert_eq!(result, Err(LobbyError::RoomFull(room_id)));
    }

    #[test]
    fn test_join_room_already_in() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");

        let result = lobby.join_room(room_id, 1, "Alice");
        assert_eq!(result, Err(LobbyError::AlreadyInRoom(1)));
    }

    #[test]
    fn test_join_nonexistent_room() {
        let mut lobby = Lobby::new();
        let result = lobby.join_room(999, 1, "Alice");
        assert_eq!(result, Err(LobbyError::RoomNotFound(999)));
    }

    #[test]
    fn test_leave_room() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.leave_room(room_id, 2).unwrap();
        let room = lobby.room(room_id).unwrap();
        assert_eq!(room.player_count(), 1);
    }

    #[test]
    fn test_leave_room_host_transfers() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.leave_room(room_id, 1).unwrap(); // host leaves
        let room = lobby.room(room_id).unwrap();
        assert_eq!(room.host, 2); // Bob is now host
    }

    #[test]
    fn test_leave_room_last_player_removes_room() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");

        lobby.leave_room(room_id, 1).unwrap();
        assert_eq!(lobby.room_count(), 0);
        assert!(lobby.room(room_id).is_none());
    }

    #[test]
    fn test_leave_room_not_in() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        let result = lobby.leave_room(room_id, 99);
        assert_eq!(result, Err(LobbyError::NotInRoom(99)));
    }

    // -- Ready state ----------------------------------------------------------

    #[test]
    fn test_set_ready() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        let room = lobby.room(room_id).unwrap();
        assert_eq!(room.find_player(1).unwrap().ready, ReadyState::Ready);
        assert_eq!(room.state, RoomState::Waiting); // not all ready yet
    }

    #[test]
    fn test_all_ready_transitions_state() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        lobby.set_ready(room_id, 2, ReadyState::Ready).unwrap();

        let room = lobby.room(room_id).unwrap();
        assert!(room.all_ready());
        assert_eq!(room.state, RoomState::AllReady);
    }

    #[test]
    fn test_unready_reverts_state() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        lobby.set_ready(room_id, 2, ReadyState::Ready).unwrap();
        assert_eq!(lobby.room(room_id).unwrap().state, RoomState::AllReady);

        lobby.set_ready(room_id, 2, ReadyState::NotReady).unwrap();
        assert_eq!(lobby.room(room_id).unwrap().state, RoomState::Waiting);
    }

    // -- Game start -----------------------------------------------------------

    #[test]
    fn test_start_game() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        lobby.set_ready(room_id, 2, ReadyState::Ready).unwrap();

        lobby.start_game(room_id, 1).unwrap();
        assert_eq!(lobby.room(room_id).unwrap().state, RoomState::InGame);
    }

    #[test]
    fn test_start_game_not_host() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        lobby.set_ready(room_id, 2, ReadyState::Ready).unwrap();

        let result = lobby.start_game(room_id, 2); // Bob is not host
        assert_eq!(result, Err(LobbyError::NotHost));
    }

    #[test]
    fn test_start_game_not_all_ready() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        // Bob not ready

        let result = lobby.start_game(room_id, 1);
        assert_eq!(result, Err(LobbyError::InvalidState(RoomState::Waiting)));
    }

    #[test]
    fn test_join_room_in_game() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        lobby.start_game(room_id, 1).unwrap();

        let result = lobby.join_room(room_id, 2, "Bob");
        assert_eq!(result, Err(LobbyError::InvalidState(RoomState::InGame)));
    }

    // -- Close / cleanup ------------------------------------------------------

    #[test]
    fn test_close_room() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");

        lobby.set_ready(room_id, 1, ReadyState::Ready).unwrap();
        lobby.start_game(room_id, 1).unwrap();
        lobby.close_room(room_id, 1).unwrap();

        assert_eq!(lobby.room(room_id).unwrap().state, RoomState::Closed);
    }

    #[test]
    fn test_close_room_not_host() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Test", 4, 1, "Alice");
        lobby.join_room(room_id, 2, "Bob").unwrap();

        let result = lobby.close_room(room_id, 2);
        assert_eq!(result, Err(LobbyError::NotHost));
    }

    #[test]
    fn test_cleanup_closed() {
        let mut lobby = Lobby::new();
        let r1 = lobby.create_room("Room1", 4, 1, "Alice");
        let r2 = lobby.create_room("Room2", 4, 2, "Bob");

        lobby.set_ready(r1, 1, ReadyState::Ready).unwrap();
        lobby.start_game(r1, 1).unwrap();
        lobby.close_room(r1, 1).unwrap();

        lobby.cleanup_closed();
        assert_eq!(lobby.room_count(), 1);
        assert!(lobby.room(r1).is_none());
        assert!(lobby.room(r2).is_some());
    }

    // -- Listing joinable rooms -----------------------------------------------

    #[test]
    fn test_list_joinable() {
        let mut lobby = Lobby::new();
        let r1 = lobby.create_room("Open", 4, 1, "Alice");
        let r2 = lobby.create_room("Full", 1, 2, "Bob"); // 1 slot, host fills it
        let r3 = lobby.create_room("Also Open", 4, 3, "Charlie");

        // r2 is full (max 1, host occupies it)
        let joinable = lobby.list_joinable();
        let joinable_ids: Vec<RoomId> = joinable.iter().map(|r| r.id).collect();
        assert!(joinable_ids.contains(&r1));
        assert!(!joinable_ids.contains(&r2));
        assert!(joinable_ids.contains(&r3));
    }

    // -- Two clients join, verify player list ---------------------------------

    #[test]
    fn test_two_clients_join_verify_player_list() {
        let mut lobby = Lobby::new();
        let room_id = lobby.create_room("Party", 4, 100, "Alice");
        lobby.join_room(room_id, 200, "Bob").unwrap();

        let room = lobby.room(room_id).unwrap();
        assert_eq!(room.player_count(), 2);

        let names = room.player_names();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));

        assert_eq!(room.find_player(100).unwrap().name, "Alice");
        assert_eq!(room.find_player(200).unwrap().name, "Bob");
    }
}
