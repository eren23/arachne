//! Application-level ECS components used by the integration systems.
//!
//! These bridge the gap between subsystem types (physics, audio, render) and
//! the ECS world.

use arachne_math::Transform;
use arachne_physics::BodyHandle;

// ---------------------------------------------------------------------------
// Transform hierarchy
// ---------------------------------------------------------------------------

/// The computed world-space transform for an entity, derived from its local
/// `Transform` and its parent chain. For root entities this equals `Transform`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform(pub Transform);

impl GlobalTransform {
    pub const IDENTITY: Self = Self(Transform::IDENTITY);

    pub fn new(transform: Transform) -> Self {
        Self(transform)
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

// ---------------------------------------------------------------------------
// Physics body component
// ---------------------------------------------------------------------------

/// Tracks the association between an ECS entity and a physics body.
#[derive(Clone, Copy, Debug)]
pub enum PhysicsBodyState {
    /// Not yet registered in the PhysicsWorld.
    Pending,
    /// Active with the given body handle.
    Active(BodyHandle),
    /// Removed from the physics world.
    Removed,
}

/// Component that marks an entity as having a rigid body in the physics world.
#[derive(Clone, Debug)]
pub struct PhysicsBody {
    /// Current state of the physics body association.
    pub state: PhysicsBodyState,
    /// Initial body configuration. Used when spawning into the physics world.
    pub body_type: arachne_physics::BodyType,
    pub mass: f32,
    pub inertia: f32,
}

impl PhysicsBody {
    pub fn dynamic(mass: f32, inertia: f32) -> Self {
        Self {
            state: PhysicsBodyState::Pending,
            body_type: arachne_physics::BodyType::Dynamic,
            mass,
            inertia,
        }
    }

    pub fn kinematic() -> Self {
        Self {
            state: PhysicsBodyState::Pending,
            body_type: arachne_physics::BodyType::Kinematic,
            mass: 0.0,
            inertia: 0.0,
        }
    }

    pub fn static_body() -> Self {
        Self {
            state: PhysicsBodyState::Pending,
            body_type: arachne_physics::BodyType::Static,
            mass: 0.0,
            inertia: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Collider component
// ---------------------------------------------------------------------------

/// ECS-side collider component. Mirrors `arachne_physics::Collider`.
pub type ColliderComponent = arachne_physics::Collider;

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

/// Marker component for the active camera entity.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub zoom: f32,
    pub active: bool,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            active: true,
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Draw call tracking (for diagnostics / tests)
// ---------------------------------------------------------------------------

/// Resource that tracks how many drawable entities were processed this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawCallCount(pub u32);
