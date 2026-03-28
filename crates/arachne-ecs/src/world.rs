use crate::archetype::{get_two_mut, Archetype, ArchetypeId, EntityLocation};
use crate::bundle::Bundle;
use crate::commands::CommandQueue;
use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::entity::{Entity, EntityAllocator};
use crate::event::EventStorage;
use crate::query::{QueryFilter, QueryIter, ReadOnlyWorldQuery, WorldQuery};
use crate::resource::ResourceMap;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

pub struct World {
    allocator: EntityAllocator,
    /// Indexed by `entity.index()`. Grows to match the allocator.
    locations: Vec<EntityLocation>,
    pub(crate) archetypes: Vec<Archetype>,
    /// Maps a *sorted* component-id set to the archetype that stores it.
    archetype_index: HashMap<Vec<ComponentId>, ArchetypeId>,
    pub(crate) registry: ComponentRegistry,
    /// Global resources (singleton values keyed by type).
    pub(crate) resources: ResourceMap,
    /// Event queues (double-buffered, type-erased).
    pub(crate) events: EventStorage,
    /// Deferred command queue (flushed by `apply_deferred`).
    pub(crate) command_queue: CommandQueue,
    /// Monotonic tick counter, incremented each frame by the schedule.
    pub(crate) tick: u32,
}

impl World {
    pub fn new() -> Self {
        Self {
            allocator: EntityAllocator::new(),
            locations: Vec::new(),
            archetypes: Vec::new(),
            archetype_index: HashMap::new(),
            registry: ComponentRegistry::new(),
            resources: ResourceMap::new(),
            events: EventStorage::new(),
            command_queue: CommandQueue::new(),
            tick: 0,
        }
    }

    // -- Entity lifecycle ----------------------------------------------------

    /// Spawn a new entity with the given component bundle.
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> Entity {
        let comp_ids = B::component_ids(&mut self.registry);
        let arch_id = self.get_or_create_archetype(&comp_ids);

        let entity = self.allocator.allocate();

        // Ensure locations vec is large enough.
        let idx = entity.index() as usize;
        if idx >= self.locations.len() {
            self.locations.resize(idx + 1, EntityLocation::INVALID);
        }

        let archetype = &mut self.archetypes[arch_id.0];
        let row = archetype.len();
        unsafe { bundle.write_to_archetype(archetype, &self.registry, self.tick) };
        archetype.push_entity(entity);

        self.locations[idx] = EntityLocation {
            archetype_id: arch_id,
            row,
        };
        entity
    }

    /// Remove an entity and drop all its components. Returns `false` if already
    /// dead.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.allocator.is_alive(entity) {
            return false;
        }
        let loc = self.locations[entity.index() as usize];
        let arch = &mut self.archetypes[loc.archetype_id.0];
        let swapped = arch.remove_entity_drop(loc.row);

        if let Some(swapped_entity) = swapped {
            self.locations[swapped_entity.index() as usize].row = loc.row;
        }

        self.locations[entity.index() as usize] = EntityLocation::INVALID;
        self.allocator.deallocate(entity);
        true
    }

    /// Is this entity handle still valid?
    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_alive(entity)
    }

    /// Number of living entities.
    #[inline]
    pub fn entity_count(&self) -> u32 {
        self.allocator.alive_count()
    }

    // -- Component access on a single entity ---------------------------------

    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        if !self.allocator.is_alive(entity) {
            return None;
        }
        let loc = self.locations[entity.index() as usize];
        let comp_id = self.registry.lookup::<T>()?;
        let arch = &self.archetypes[loc.archetype_id.0];
        let col = arch.column_index(comp_id)?;
        let ptr = arch.column_data_ptr(col);
        Some(unsafe { &*(ptr.add(loc.row * std::mem::size_of::<T>()) as *const T) })
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.allocator.is_alive(entity) {
            return None;
        }
        let loc = self.locations[entity.index() as usize];
        let comp_id = self.registry.lookup::<T>()?;
        let arch = &mut self.archetypes[loc.archetype_id.0];
        let col = arch.column_index(comp_id)?;
        // Mark component as changed for change detection.
        let tick = self.tick;
        arch.set_change_tick(col, loc.row, tick);
        let ptr = arch.column_data_ptr(col);
        Some(unsafe { &mut *(ptr.add(loc.row * std::mem::size_of::<T>()) as *mut T) })
    }

    // -- Insert / remove single component (archetype migration) ---------------

    /// Add (or overwrite) a component on an existing entity.
    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        assert!(self.allocator.is_alive(entity), "entity is dead");
        let comp_id = self.registry.get_or_register::<T>();
        let loc = self.locations[entity.index() as usize];
        let src_arch_id = loc.archetype_id;
        let tick = self.tick;

        // Already has this component? Overwrite in place.
        if let Some(col) = self.archetypes[src_arch_id.0].column_index(comp_id) {
            unsafe {
                self.archetypes[src_arch_id.0].replace_component(
                    col,
                    loc.row,
                    &component as *const T as *const u8,
                    tick,
                );
            }
            std::mem::forget(component);
            return;
        }

        // Compute target archetype.
        let dst_arch_id = self.get_or_create_add_archetype(src_arch_id, comp_id);

        // Move existing data to new archetype.
        let (src, dst) = get_two_mut(&mut self.archetypes, src_arch_id.0, dst_arch_id.0);
        let new_row = dst.len();

        // Copy existing columns (preserving their original ticks).
        for (src_col, &src_comp_id) in src.component_ids().to_vec().iter().enumerate() {
            let change_t = src.change_tick(src_col, loc.row);
            let added_t = src.added_tick(src_col, loc.row);
            let dst_col = dst.column_index(src_comp_id).unwrap();
            unsafe {
                let ptr = src.column_data_ptr(src_col).add(loc.row * src.column_item_size(src_col));
                dst.push_component_data(dst_col, ptr, 0);
                // Fix the ticks to preserve source values.
                let ct_ptr = dst.change_ticks_slice(dst_col).as_ptr() as *mut u32;
                let at_ptr = dst.added_ticks_slice(dst_col).as_ptr() as *mut u32;
                *ct_ptr.add(new_row) = change_t;
                *at_ptr.add(new_row) = added_t;
            }
        }

        // Push the new component (with current tick as both change + added).
        let dst_col = dst.column_index(comp_id).unwrap();
        unsafe {
            dst.push_component_data(dst_col, &component as *const T as *const u8, tick);
        }
        std::mem::forget(component);

        dst.push_entity(entity);

        // Remove from source without dropping (data was moved).
        let swapped = unsafe { src.remove_entity_forget(loc.row) };
        if let Some(swapped_entity) = swapped {
            self.locations[swapped_entity.index() as usize].row = loc.row;
        }

        self.locations[entity.index() as usize] = EntityLocation {
            archetype_id: dst_arch_id,
            row: new_row,
        };
    }

    /// Remove a component from an entity, returning it. Returns `None` if the
    /// entity doesn't have the component.
    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        assert!(self.allocator.is_alive(entity), "entity is dead");
        let comp_id = self.registry.lookup::<T>()?;
        let loc = self.locations[entity.index() as usize];
        let src_arch_id = loc.archetype_id;

        if self.archetypes[src_arch_id.0].column_index(comp_id).is_none() {
            return None;
        }

        // Read the value before moving.
        let value: T = unsafe {
            let col = self.archetypes[src_arch_id.0].column_index(comp_id).unwrap();
            let ptr = self.archetypes[src_arch_id.0]
                .column_data_ptr(col)
                .add(loc.row * std::mem::size_of::<T>());
            std::ptr::read(ptr as *const T)
        };

        // Compute target archetype (without this component).
        let dst_arch_id = self.get_or_create_remove_archetype(src_arch_id, comp_id);

        let (src, dst) = get_two_mut(&mut self.archetypes, src_arch_id.0, dst_arch_id.0);
        let new_row = dst.len();

        // Copy columns that exist in destination (preserving ticks).
        for (src_col, &src_comp_id) in src.component_ids().to_vec().iter().enumerate() {
            if src_comp_id == comp_id {
                continue; // Skip the removed component.
            }
            let change_t = src.change_tick(src_col, loc.row);
            let added_t = src.added_tick(src_col, loc.row);
            let dst_col = dst.column_index(src_comp_id).unwrap();
            unsafe {
                let ptr = src.column_data_ptr(src_col).add(loc.row * src.column_item_size(src_col));
                dst.push_component_data(dst_col, ptr, 0);
                // Fix ticks to preserve source values.
                let ct_ptr = dst.change_ticks_slice(dst_col).as_ptr() as *mut u32;
                let at_ptr = dst.added_ticks_slice(dst_col).as_ptr() as *mut u32;
                *ct_ptr.add(new_row) = change_t;
                *at_ptr.add(new_row) = added_t;
            }
        }
        dst.push_entity(entity);

        // Remove from source without drop (data was moved / already read).
        let swapped = unsafe { src.remove_entity_forget(loc.row) };
        if let Some(swapped_entity) = swapped {
            self.locations[swapped_entity.index() as usize].row = loc.row;
        }

        self.locations[entity.index() as usize] = EntityLocation {
            archetype_id: dst_arch_id,
            row: new_row,
        };
        Some(value)
    }

    // -- Resource API --------------------------------------------------------

    /// Insert a resource (singleton value).
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, resource: T) {
        self.resources.insert(resource);
    }

    /// Get an immutable reference to a resource. Panics if not found.
    pub fn get_resource<T: 'static + Send + Sync>(&self) -> &T {
        self.resources.get::<T>()
    }

    /// Get a mutable reference to a resource. Panics if not found.
    pub fn get_resource_mut<T: 'static + Send + Sync>(&mut self) -> &mut T {
        self.resources.get_mut::<T>()
    }

    /// Remove a resource, returning it.
    pub fn remove_resource<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }

    /// Check if a resource exists.
    pub fn has_resource<T: 'static + Send + Sync>(&self) -> bool {
        self.resources.contains::<T>()
    }

    // -- Event API -----------------------------------------------------------

    /// Register an event type. Must be called before EventReader/EventWriter
    /// system params can be used for this type.
    pub fn add_event<T: 'static + Send + Sync>(&mut self) {
        self.events.register::<T>();
    }

    // -- Queries -------------------------------------------------------------

    /// Iterate entities matching a read-only query.
    pub fn query<Q: ReadOnlyWorldQuery>(&self) -> QueryIter<'_, Q, ()> {
        self.build_query_iter()
    }

    /// Iterate entities matching a (possibly mutable) query.
    pub fn query_mut<Q: WorldQuery>(&mut self) -> QueryIter<'_, Q, ()> {
        self.build_query_iter()
    }

    /// Iterate with a read-only query + filter.
    pub fn query_filtered<Q: ReadOnlyWorldQuery, F: QueryFilter>(
        &self,
    ) -> QueryIter<'_, Q, F> {
        self.build_query_iter()
    }

    /// Iterate with a (possibly mutable) query + filter.
    pub fn query_filtered_mut<Q: WorldQuery, F: QueryFilter>(
        &mut self,
    ) -> QueryIter<'_, Q, F> {
        self.build_query_iter()
    }

    pub(crate) fn build_query_iter<Q: WorldQuery, F: QueryFilter>(&self) -> QueryIter<'_, Q, F> {
        let query_state = Q::init_state(&self.registry);
        let filter_state = F::init_state(&self.registry);
        QueryIter::new(&self.archetypes, query_state, filter_state, self.tick)
    }

    // -- Archetype management (internal) -------------------------------------

    fn get_or_create_archetype(&mut self, comp_ids: &[ComponentId]) -> ArchetypeId {
        if let Some(&id) = self.archetype_index.get(comp_ids) {
            return id;
        }
        let id = ArchetypeId(self.archetypes.len());
        self.archetypes
            .push(Archetype::new(id, comp_ids.to_vec(), &self.registry));
        self.archetype_index.insert(comp_ids.to_vec(), id);
        id
    }

    fn get_or_create_add_archetype(
        &mut self,
        src: ArchetypeId,
        component: ComponentId,
    ) -> ArchetypeId {
        if let Some(cached) = self.archetypes[src.0].add_edge(component) {
            return cached;
        }
        let mut new_ids: Vec<ComponentId> = self.archetypes[src.0].component_ids().to_vec();
        // Insert in sorted order.
        match new_ids.binary_search(&component) {
            Ok(_) => unreachable!("component already in archetype"),
            Err(pos) => new_ids.insert(pos, component),
        }
        let target = self.get_or_create_archetype(&new_ids);
        self.archetypes[src.0].set_add_edge(component, target);
        self.archetypes[target.0].set_remove_edge(component, src);
        target
    }

    fn get_or_create_remove_archetype(
        &mut self,
        src: ArchetypeId,
        component: ComponentId,
    ) -> ArchetypeId {
        if let Some(cached) = self.archetypes[src.0].remove_edge(component) {
            return cached;
        }
        let new_ids: Vec<ComponentId> = self.archetypes[src.0]
            .component_ids()
            .iter()
            .copied()
            .filter(|&id| id != component)
            .collect();
        let target = self.get_or_create_archetype(&new_ids);
        self.archetypes[src.0].set_remove_edge(component, target);
        self.archetypes[target.0].set_add_edge(component, src);
        target
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
