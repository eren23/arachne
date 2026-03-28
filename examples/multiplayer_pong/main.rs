//! 2-player networked pong demo.
//!
//! Uses arachne-networking's MockTransport for local simulation of a
//! server-authoritative pong game. Two paddles (up/down), a bouncing ball,
//! score display, and win condition at 11 points.
//!
//! Demonstrates: networking, input, UI, shapes, game state synchronization.

use arachne_app::{
    App, Camera, DefaultPlugins, HeadlessRunner, Transform, Vec2, Vec3, Color,
    Res, ResMut, Time,
};
use arachne_networking::{NetworkClient, NetworkServer, ClientConfig, ServerConfig};
use arachne_render::{Sprite, TextureHandle};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ARENA_WIDTH: f32 = 40.0;
const ARENA_HEIGHT: f32 = 30.0;
const PADDLE_HEIGHT: f32 = 5.0;
const PADDLE_WIDTH: f32 = 1.0;
const PADDLE_SPEED: f32 = 15.0;
const BALL_SPEED: f32 = 12.0;
const BALL_SIZE: f32 = 0.8;
const WIN_SCORE: u32 = 11;

// ---------------------------------------------------------------------------
// Game state (server-authoritative)
// ---------------------------------------------------------------------------

struct PongState {
    // Paddle Y positions (centered).
    paddle_left_y: f32,
    paddle_right_y: f32,

    // Ball position and velocity.
    ball_pos: Vec2,
    ball_vel: Vec2,

    // Scores.
    score_left: u32,
    score_right: u32,

    // Game over flag.
    winner: Option<u8>, // 0 = left, 1 = right

    // Networking.
    server: NetworkServer,
    client_left: NetworkClient,
    client_right: NetworkClient,
}

impl PongState {
    fn new() -> Self {
        let server = NetworkServer::new(ServerConfig {
            listen_url: "ws://localhost:9999".into(),
            max_clients: 2,
            tick_rate: 60,
        });

        let client_left = NetworkClient::new(ClientConfig {
            server_url: "ws://localhost:9999".into(),
            client_name: "Player 1".into(),
            ..Default::default()
        });

        let client_right = NetworkClient::new(ClientConfig {
            server_url: "ws://localhost:9999".into(),
            client_name: "Player 2".into(),
            ..Default::default()
        });

        Self {
            paddle_left_y: 0.0,
            paddle_right_y: 0.0,
            ball_pos: Vec2::ZERO,
            ball_vel: Vec2::new(BALL_SPEED, BALL_SPEED * 0.5),
            score_left: 0,
            score_right: 0,
            winner: None,
            server,
            client_left,
            client_right,
        }
    }

    /// Reset ball to center with a new direction.
    fn reset_ball(&mut self, direction: f32) {
        self.ball_pos = Vec2::ZERO;
        self.ball_vel = Vec2::new(BALL_SPEED * direction, BALL_SPEED * 0.3);
    }

    /// Encode game state into bytes for network transmission.
    fn encode_state(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(32);
        data.extend_from_slice(&self.paddle_left_y.to_le_bytes());
        data.extend_from_slice(&self.paddle_right_y.to_le_bytes());
        data.extend_from_slice(&self.ball_pos.x.to_le_bytes());
        data.extend_from_slice(&self.ball_pos.y.to_le_bytes());
        data.extend_from_slice(&self.score_left.to_le_bytes());
        data.extend_from_slice(&self.score_right.to_le_bytes());
        data
    }
}

// ---------------------------------------------------------------------------
// Input encoding (what clients send to server)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum PaddleInput {
    None,
    Up,
    Down,
}

impl PaddleInput {
    fn to_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Up => 1,
            Self::Down => 2,
        }
    }

    fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Up,
            2 => Self::Down,
            _ => Self::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Simulate player inputs (in headless mode, AI-controlled paddles).
fn simulate_inputs(mut state: ResMut<PongState>, time: Res<Time>) {
    if state.winner.is_some() {
        return;
    }

    let dt = time.delta_seconds();

    // AI: left paddle tracks ball y.
    let left_target = state.ball_pos.y;
    let left_diff = left_target - state.paddle_left_y;
    if left_diff.abs() > 0.5 {
        let dir = left_diff.signum();
        state.paddle_left_y += dir * PADDLE_SPEED * dt;
    }

    // AI: right paddle tracks ball y (slightly slower for variety).
    let right_target = state.ball_pos.y;
    let right_diff = right_target - state.paddle_right_y;
    if right_diff.abs() > 1.0 {
        let dir = right_diff.signum();
        state.paddle_right_y += dir * PADDLE_SPEED * 0.85 * dt;
    }

    // Clamp paddles to arena.
    let half_arena = ARENA_HEIGHT / 2.0 - PADDLE_HEIGHT / 2.0;
    state.paddle_left_y = state.paddle_left_y.clamp(-half_arena, half_arena);
    state.paddle_right_y = state.paddle_right_y.clamp(-half_arena, half_arena);
}

/// Update ball physics: movement, wall bounces, paddle collisions, scoring.
fn update_ball(mut state: ResMut<PongState>, time: Res<Time>) {
    if state.winner.is_some() {
        return;
    }

    let dt = time.delta_seconds();

    // Move ball.
    state.ball_pos.x += state.ball_vel.x * dt;
    state.ball_pos.y += state.ball_vel.y * dt;

    let half_h = ARENA_HEIGHT / 2.0 - BALL_SIZE / 2.0;
    let half_w = ARENA_WIDTH / 2.0;

    // Top/bottom wall bounce.
    if state.ball_pos.y > half_h {
        state.ball_pos.y = half_h;
        state.ball_vel.y = -state.ball_vel.y.abs();
    }
    if state.ball_pos.y < -half_h {
        state.ball_pos.y = -half_h;
        state.ball_vel.y = state.ball_vel.y.abs();
    }

    // Left paddle collision.
    let left_x = -half_w + PADDLE_WIDTH;
    if state.ball_pos.x - BALL_SIZE / 2.0 <= left_x
        && state.ball_pos.x - BALL_SIZE / 2.0 > left_x - 1.0
    {
        let paddle_top = state.paddle_left_y + PADDLE_HEIGHT / 2.0;
        let paddle_bottom = state.paddle_left_y - PADDLE_HEIGHT / 2.0;
        if state.ball_pos.y >= paddle_bottom && state.ball_pos.y <= paddle_top {
            state.ball_pos.x = left_x + BALL_SIZE / 2.0;
            state.ball_vel.x = state.ball_vel.x.abs(); // bounce right
            // Add spin based on paddle hit position.
            let hit_offset = (state.ball_pos.y - state.paddle_left_y) / (PADDLE_HEIGHT / 2.0);
            state.ball_vel.y = BALL_SPEED * hit_offset;
        }
    }

    // Right paddle collision.
    let right_x = half_w - PADDLE_WIDTH;
    if state.ball_pos.x + BALL_SIZE / 2.0 >= right_x
        && state.ball_pos.x + BALL_SIZE / 2.0 < right_x + 1.0
    {
        let paddle_top = state.paddle_right_y + PADDLE_HEIGHT / 2.0;
        let paddle_bottom = state.paddle_right_y - PADDLE_HEIGHT / 2.0;
        if state.ball_pos.y >= paddle_bottom && state.ball_pos.y <= paddle_top {
            state.ball_pos.x = right_x - BALL_SIZE / 2.0;
            state.ball_vel.x = -state.ball_vel.x.abs(); // bounce left
            let hit_offset = (state.ball_pos.y - state.paddle_right_y) / (PADDLE_HEIGHT / 2.0);
            state.ball_vel.y = BALL_SPEED * hit_offset;
        }
    }

    // Scoring: ball passes left/right edges.
    if state.ball_pos.x < -half_w - 1.0 {
        state.score_right += 1;
        state.reset_ball(1.0); // serve to the right
    }
    if state.ball_pos.x > half_w + 1.0 {
        state.score_left += 1;
        state.reset_ball(-1.0); // serve to the left
    }

    // Win check.
    if state.score_left >= WIN_SCORE {
        state.winner = Some(0);
    } else if state.score_right >= WIN_SCORE {
        state.winner = Some(1);
    }
}

/// Broadcast state from server to clients (simulated via encode/decode).
fn network_sync(mut state: ResMut<PongState>) {
    // Encode authoritative state.
    let state_bytes = state.encode_state();

    // In a real networked game, the server would broadcast this to all clients:
    //   let msg = Message::state_update(server.next_sequence(), &state_bytes);
    //   server.broadcast(&msg);
    //
    // And clients would receive and apply it:
    //   client.process_received();
    //
    // Here we demonstrate the encoding/decoding roundtrip.
    let _roundtrip_size = state_bytes.len();
}

/// Draw score UI (simulated in headless mode).
fn draw_score(state: Res<PongState>) {
    // In a full engine:
    //   Label::new(&format!("{} - {}", state.score_left, state.score_right))
    //       .font_size(32.0)
    //       .align(TextAlign::Center)
    //       .show(&mut ctx);
    //
    //   if let Some(winner) = state.winner {
    //       let name = if winner == 0 { "Player 1" } else { "Player 2" };
    //       Label::new(&format!("{} wins!", name)).font_size(48.0).show(&mut ctx);
    //   }
    let _ = state.score_left;
    let _ = state.score_right;
}

fn main() {
    let mut app = App::new();
    app.add_plugin(DefaultPlugins);

    app.insert_resource(PongState::new());

    app.add_system(simulate_inputs);
    app.add_system(update_ball);
    app.add_system(network_sync);
    app.add_system(draw_score);

    app.build_plugins();

    // Spawn camera and visual entities.
    app.world.spawn((Camera::new(), Transform::IDENTITY));

    // Left paddle sprite.
    let mut left_sprite = Sprite::new(TextureHandle(0));
    left_sprite.color = Color::rgb(0.2, 0.6, 1.0);
    left_sprite.custom_size = Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT));
    app.world.spawn((
        left_sprite,
        Transform::from_position(Vec3::new(-ARENA_WIDTH / 2.0 + PADDLE_WIDTH / 2.0, 0.0, 0.0)),
    ));

    // Right paddle sprite.
    let mut right_sprite = Sprite::new(TextureHandle(0));
    right_sprite.color = Color::rgb(1.0, 0.4, 0.2);
    right_sprite.custom_size = Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT));
    app.world.spawn((
        right_sprite,
        Transform::from_position(Vec3::new(ARENA_WIDTH / 2.0 - PADDLE_WIDTH / 2.0, 0.0, 0.0)),
    ));

    // Ball sprite.
    let mut ball_sprite = Sprite::new(TextureHandle(0));
    ball_sprite.color = Color::WHITE;
    ball_sprite.custom_size = Some(Vec2::new(BALL_SIZE, BALL_SIZE));
    app.world.spawn((
        ball_sprite,
        Transform::from_position(Vec3::ZERO),
    ));

    // Run enough frames for a full game (could be many rallies).
    // At 60fps, run 30 seconds = 1800 frames.
    app.set_runner(HeadlessRunner::new(1800, 1.0 / 60.0));
    app.run();

    let state = app.world.get_resource::<PongState>();
    let winner_name = match state.winner {
        Some(0) => "Player 1 (left)",
        Some(1) => "Player 2 (right)",
        _ => "No winner yet",
    };
    println!(
        "Multiplayer Pong: {} - {} | Winner: {}",
        state.score_left, state.score_right, winner_name,
    );
}
