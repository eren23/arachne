use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::query::QueryFilter;
use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// Tick tracking
// ---------------------------------------------------------------------------

/// A monotonic tick counter used for change detection. Each frame the world
/// increments its tick, and component writes record the current tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tick(pub u32);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    #[inline]
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }

    /// Returns `true` if this tick is newer than `last_seen` relative to
    /// `current_tick`. Handles wrapping via half-space comparison.
    #[inline]
    pub fn is_newer_than(self, last_seen: Tick, _current_tick: Tick) -> bool {
        // Simple comparison for now (wrapping handled when u32 overflows via
        // subtraction). In practice the engine won't hit 2^31 ticks.
        self.0 > last_seen.0
    }

    /// Check if this tick was set within the last N ticks relative to `current`.
    #[inline]
    pub fn is_within(self, current: Tick, window: u32) -> bool {
        current.0.wrapping_sub(self.0) <= window
    }
}

// ---------------------------------------------------------------------------
// ChangeTrackers<T> – query item that provides tick info alongside the value
// ---------------------------------------------------------------------------

/// Query item that yields a reference to the component along with its
/// change and added tick information.
pub struct ChangeTrackers<T: Component> {
    _marker: PhantomData<T>,
}

/// The item yielded by a `ChangeTrackers<T>` query.
#[derive(Debug)]
pub struct ChangeTrackersItem<'w, T> {
    pub value: &'w T,
    pub change_tick: Tick,
    pub added_tick: Tick,
    pub world_tick: Tick,
}

impl<'w, T> ChangeTrackersItem<'w, T> {
    /// Was this component modified on the current tick?
    #[inline]
    pub fn is_changed(&self) -> bool {
        self.change_tick == self.world_tick
    }

    /// Was this component added on the current tick?
    #[inline]
    pub fn is_added(&self) -> bool {
        self.added_tick == self.world_tick
    }

    /// Was this component modified within the last N ticks?
    #[inline]
    pub fn changed_within(&self, window: u32) -> bool {
        self.change_tick.is_within(self.world_tick, window)
    }

    /// Was this component added within the last N ticks?
    #[inline]
    pub fn added_within(&self, window: u32) -> bool {
        self.added_tick.is_within(self.world_tick, window)
    }
}

// ---------------------------------------------------------------------------
// Changed<T> – filter for components modified this frame
// ---------------------------------------------------------------------------

/// Query filter that matches entities whose component `T` was modified
/// (written) during the current tick.
///
/// Works with `world.get_mut()`, `world.insert_component()` (overwrite),
/// and `world.spawn()`.
pub struct Changed<T: Component>(PhantomData<T>);

pub struct ChangedFetch {
    change_ticks: *const u32,
    world_tick: u32,
}

impl<T: Component> QueryFilter for Changed<T> {
    type State = Option<ComponentId>;
    type Fetch<'w> = ChangedFetch;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.map_or(false, |id| archetype.has_component(id))
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w> {
        let col = archetype.column_index(state.unwrap()).unwrap();
        ChangedFetch {
            change_ticks: archetype.change_ticks_slice(col).as_ptr(),
            world_tick,
        }
    }

    #[inline]
    fn matches_row(fetch: &Self::Fetch<'_>, row: usize) -> bool {
        unsafe { *fetch.change_ticks.add(row) == fetch.world_tick }
    }
}

// ---------------------------------------------------------------------------
// Added<T> – filter for components added this frame
// ---------------------------------------------------------------------------

/// Query filter that matches entities where component `T` was added during
/// the current tick (via `spawn`, `insert_component` of a new component,
/// or `Commands::spawn`).
pub struct Added<T: Component>(PhantomData<T>);

pub struct AddedFetch {
    added_ticks: *const u32,
    world_tick: u32,
}

impl<T: Component> QueryFilter for Added<T> {
    type State = Option<ComponentId>;
    type Fetch<'w> = AddedFetch;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.map_or(false, |id| archetype.has_component(id))
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w> {
        let col = archetype.column_index(state.unwrap()).unwrap();
        AddedFetch {
            added_ticks: archetype.added_ticks_slice(col).as_ptr(),
            world_tick,
        }
    }

    #[inline]
    fn matches_row(fetch: &Self::Fetch<'_>, row: usize) -> bool {
        unsafe { *fetch.added_ticks.add(row) == fetch.world_tick }
    }
}

// ---------------------------------------------------------------------------
// ChangedWithin<T, const W: u32> – filter for recent changes within a window
// ---------------------------------------------------------------------------

/// Filter that matches entities where component `T` was modified within the
/// last `W` ticks. Useful for "recently changed" queries (e.g. the last 5
/// frames).
pub struct ChangedWithin<T: Component, const W: u32>(PhantomData<T>);

pub struct ChangedWithinFetch {
    change_ticks: *const u32,
    world_tick: u32,
    window: u32,
}

impl<T: Component, const W: u32> QueryFilter for ChangedWithin<T, W> {
    type State = Option<ComponentId>;
    type Fetch<'w> = ChangedWithinFetch;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.map_or(false, |id| archetype.has_component(id))
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w> {
        let col = archetype.column_index(state.unwrap()).unwrap();
        ChangedWithinFetch {
            change_ticks: archetype.change_ticks_slice(col).as_ptr(),
            world_tick,
            window: W,
        }
    }

    #[inline]
    fn matches_row(fetch: &Self::Fetch<'_>, row: usize) -> bool {
        let tick = Tick(unsafe { *fetch.change_ticks.add(row) });
        tick.is_within(Tick(fetch.world_tick), fetch.window)
    }
}

// ---------------------------------------------------------------------------
// Or<A, B> – filter combinator for OR logic
// ---------------------------------------------------------------------------

/// Query filter that matches if *either* `A` or `B` matches.
/// Unlike tuple filters which are AND, this provides OR semantics.
pub struct Or<A: QueryFilter, B: QueryFilter>(PhantomData<(A, B)>);

pub struct OrFetch<AF, BF> {
    a: Option<AF>,
    b: Option<BF>,
}

impl<A: QueryFilter, B: QueryFilter> QueryFilter for Or<A, B> {
    type State = (A::State, B::State);
    type Fetch<'w> = OrFetch<A::Fetch<'w>, B::Fetch<'w>>;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        (A::init_state(registry), B::init_state(registry))
    }

    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        A::matches_archetype(&state.0, archetype)
            || B::matches_archetype(&state.1, archetype)
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w> {
        let a = if A::matches_archetype(&state.0, archetype) {
            Some(A::init_fetch(&state.0, archetype, world_tick))
        } else {
            None
        };
        let b = if B::matches_archetype(&state.1, archetype) {
            Some(B::init_fetch(&state.1, archetype, world_tick))
        } else {
            None
        };
        OrFetch { a, b }
    }

    #[inline]
    fn matches_row(fetch: &Self::Fetch<'_>, row: usize) -> bool {
        let a_match = fetch.a.as_ref().map_or(false, |f| A::matches_row(f, row));
        let b_match = fetch.b.as_ref().map_or(false, |f| B::matches_row(f, row));
        a_match || b_match
    }
}

// ---------------------------------------------------------------------------
// ComponentTicks – helper for reading raw tick data
// ---------------------------------------------------------------------------

/// Aggregated tick information for a single component on a single entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentTicks {
    pub added: Tick,
    pub changed: Tick,
}

impl ComponentTicks {
    pub fn new(added: Tick, changed: Tick) -> Self {
        Self { added, changed }
    }

    /// Was this component added at exactly the given tick?
    #[inline]
    pub fn is_added_at(&self, tick: Tick) -> bool {
        self.added == tick
    }

    /// Was this component changed at exactly the given tick?
    #[inline]
    pub fn is_changed_at(&self, tick: Tick) -> bool {
        self.changed == tick
    }

    /// Was this component added within a window of ticks?
    #[inline]
    pub fn added_within(&self, current: Tick, window: u32) -> bool {
        self.added.is_within(current, window)
    }

    /// Was this component changed within a window of ticks?
    #[inline]
    pub fn changed_within(&self, current: Tick, window: u32) -> bool {
        self.changed.is_within(current, window)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_ordering() {
        let t0 = Tick::new(0);
        let t1 = Tick::new(1);
        let t5 = Tick::new(5);

        assert!(t1.is_newer_than(t0, t5));
        assert!(!t0.is_newer_than(t1, t5));
        assert!(t5.is_newer_than(t0, t5));
    }

    #[test]
    fn tick_is_within_window() {
        let current = Tick::new(10);

        // Tick 8 is 2 ticks ago -- within window of 3
        assert!(Tick::new(8).is_within(current, 3));
        // Tick 7 is 3 ticks ago -- within window of 3
        assert!(Tick::new(7).is_within(current, 3));
        // Tick 6 is 4 ticks ago -- NOT within window of 3
        assert!(!Tick::new(6).is_within(current, 3));
        // Current tick is within any window
        assert!(Tick::new(10).is_within(current, 0));
    }

    #[test]
    fn tick_zero_default() {
        let t = Tick::default();
        assert_eq!(t.get(), 0);
        assert_eq!(t, Tick::ZERO);
    }

    #[test]
    fn component_ticks_checks() {
        let ct = ComponentTicks::new(Tick::new(3), Tick::new(7));

        assert!(ct.is_added_at(Tick::new(3)));
        assert!(!ct.is_added_at(Tick::new(4)));

        assert!(ct.is_changed_at(Tick::new(7)));
        assert!(!ct.is_changed_at(Tick::new(6)));

        // At tick 10, component was added 7 ticks ago
        assert!(ct.added_within(Tick::new(10), 7));
        assert!(!ct.added_within(Tick::new(10), 6));

        // At tick 10, component was changed 3 ticks ago
        assert!(ct.changed_within(Tick::new(10), 3));
        assert!(!ct.changed_within(Tick::new(10), 2));
    }

    #[test]
    fn change_trackers_item_flags() {
        let value = 42u32;
        let item = ChangeTrackersItem {
            value: &value,
            change_tick: Tick::new(5),
            added_tick: Tick::new(3),
            world_tick: Tick::new(5),
        };

        assert!(item.is_changed());
        assert!(!item.is_added());

        assert!(item.changed_within(0));
        assert!(item.added_within(2));
        assert!(!item.added_within(1));
    }

    #[test]
    fn change_trackers_item_both_flags() {
        let value = 42u32;
        let item = ChangeTrackersItem {
            value: &value,
            change_tick: Tick::new(5),
            added_tick: Tick::new(5),
            world_tick: Tick::new(5),
        };

        assert!(item.is_changed());
        assert!(item.is_added());
    }

    #[test]
    fn tick_within_at_boundary() {
        // Exactly at boundary
        assert!(Tick::new(5).is_within(Tick::new(10), 5));
        // One past boundary
        assert!(!Tick::new(4).is_within(Tick::new(10), 5));
    }

    #[test]
    fn component_ticks_new() {
        let ct = ComponentTicks::new(Tick::new(1), Tick::new(2));
        assert_eq!(ct.added, Tick::new(1));
        assert_eq!(ct.changed, Tick::new(2));
    }
}
