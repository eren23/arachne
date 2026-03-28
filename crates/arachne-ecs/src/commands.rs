use crate::bundle::Bundle;
use crate::component::Component;
use crate::entity::Entity;
use crate::world::World;

// ---------------------------------------------------------------------------
// CommandQueue – deferred world mutations
// ---------------------------------------------------------------------------

pub struct CommandQueue {
    commands: Vec<Box<dyn FnOnce(&mut World) + Send>>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    #[inline]
    pub fn push(&mut self, command: impl FnOnce(&mut World) + Send + 'static) {
        self.commands.push(Box::new(command));
    }

    /// Apply all buffered commands to the world in order.
    pub fn apply(&mut self, world: &mut World) {
        for cmd in self.commands.drain(..) {
            cmd(world);
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Commands – system parameter for deferred mutations
// ---------------------------------------------------------------------------

/// Deferred command buffer. Pushed commands are applied at the next
/// `apply_deferred()` call (between schedule stages).
pub struct Commands {
    queue: *mut CommandQueue,
}

// SAFETY: Commands is only alive during system execution with exclusive World
// access. The raw pointer is to World's command_queue.
unsafe impl Send for Commands {}
unsafe impl Sync for Commands {}

impl Commands {
    pub(crate) fn new(queue: *mut CommandQueue) -> Self {
        Self { queue }
    }

    /// Spawn a new entity with the given bundle.
    pub fn spawn<B: Bundle + Send + 'static>(&mut self, bundle: B) {
        unsafe {
            (*self.queue).push(move |world: &mut World| {
                world.spawn(bundle);
            });
        }
    }

    /// Despawn an entity.
    pub fn despawn(&mut self, entity: Entity) {
        unsafe {
            (*self.queue).push(move |world: &mut World| {
                world.despawn(entity);
            });
        }
    }

    /// Insert a component on an existing entity (or overwrite).
    pub fn insert<T: Component + Send + 'static>(&mut self, entity: Entity, component: T) {
        unsafe {
            (*self.queue).push(move |world: &mut World| {
                world.insert_component(entity, component);
            });
        }
    }

    /// Remove a component from an entity.
    pub fn remove<T: Component>(&mut self, entity: Entity) {
        unsafe {
            (*self.queue).push(move |world: &mut World| {
                world.remove_component::<T>(entity);
            });
        }
    }

    /// Insert a resource into the world.
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, resource: T) {
        unsafe {
            (*self.queue).push(move |world: &mut World| {
                world.insert_resource(resource);
            });
        }
    }

    /// Get an `EntityCommands` builder for chaining operations on one entity.
    pub fn entity(&mut self, entity: Entity) -> EntityCommands<'_> {
        EntityCommands {
            entity,
            commands: self,
        }
    }
}

// ---------------------------------------------------------------------------
// EntityCommands – chained operations on a single entity
// ---------------------------------------------------------------------------

pub struct EntityCommands<'a> {
    entity: Entity,
    commands: &'a mut Commands,
}

impl<'a> EntityCommands<'a> {
    /// Insert a component (or overwrite existing).
    pub fn insert<T: Component + Send + 'static>(self, component: T) -> Self {
        self.commands.insert(self.entity, component);
        self
    }

    /// Remove a component.
    pub fn remove<T: Component>(self) -> Self {
        self.commands.remove::<T>(self.entity);
        self
    }

    /// Despawn the entity.
    pub fn despawn(self) {
        self.commands.despawn(self.entity);
    }

    /// Get the entity id.
    pub fn id(&self) -> Entity {
        self.entity
    }
}
