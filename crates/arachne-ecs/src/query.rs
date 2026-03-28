use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::entity::Entity;
use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// WorldQuery – the core fetch trait
// ---------------------------------------------------------------------------

/// Describes how to fetch data from archetypes during query iteration.
///
/// # Safety
/// Implementors must truthfully report access patterns. A `ReadOnly` impl must
/// never write through the pointers it obtains.
pub unsafe trait WorldQuery {
    type Item<'w>;
    type Fetch<'w>;
    type State;

    fn init_state(registry: &ComponentRegistry) -> Self::State;
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool;
    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w>;

    /// # Safety
    /// `row` must be in bounds for the archetype that produced `fetch`.
    unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w>;
}

/// Marker: the query never mutates data, so `&World` suffices.
///
/// # Safety
/// Only implement for queries that truly never write.
pub unsafe trait ReadOnlyWorldQuery: WorldQuery {}

// ---------------------------------------------------------------------------
// Fetch helper structs
// ---------------------------------------------------------------------------

pub struct ReadFetch<T> {
    ptr: *const u8,
    size: usize,
    _marker: PhantomData<T>,
}

pub struct WriteFetch<T> {
    ptr: *mut u8,
    size: usize,
    _marker: PhantomData<T>,
}

pub struct OptionReadFetch<T> {
    ptr: Option<*const u8>,
    size: usize,
    _marker: PhantomData<T>,
}

pub struct OptionWriteFetch<T> {
    ptr: Option<*mut u8>,
    size: usize,
    _marker: PhantomData<T>,
}

pub struct EntityFetch<'w> {
    entities: &'w [Entity],
}

// ---------------------------------------------------------------------------
// WorldQuery impls for primitive fetch types
// ---------------------------------------------------------------------------

// -- &T (immutable component reference) --------------------------------------

unsafe impl<T: Component> WorldQuery for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = ReadFetch<T>;
    type State = Option<ComponentId>;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.map_or(false, |id| archetype.has_component(id))
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, _world_tick: u32) -> Self::Fetch<'w> {
        let id = state.unwrap();
        let col = archetype.column_index(id).unwrap();
        ReadFetch {
            ptr: archetype.column_data_ptr(col),
            size: std::mem::size_of::<T>(),
            _marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        &*(fetch.ptr.add(row * fetch.size) as *const T)
    }
}

unsafe impl<T: Component> ReadOnlyWorldQuery for &T {}

// -- &mut T (mutable component reference) ------------------------------------

unsafe impl<T: Component> WorldQuery for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = WriteFetch<T>;
    type State = Option<ComponentId>;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.map_or(false, |id| archetype.has_component(id))
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, _world_tick: u32) -> Self::Fetch<'w> {
        let id = state.unwrap();
        let col = archetype.column_index(id).unwrap();
        WriteFetch {
            ptr: archetype.column_data_ptr(col),
            size: std::mem::size_of::<T>(),
            _marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        &mut *(fetch.ptr.add(row * fetch.size) as *mut T)
    }
}

// -- Option<&T> --------------------------------------------------------------

unsafe impl<T: Component> WorldQuery for Option<&T> {
    type Item<'w> = Option<&'w T>;
    type Fetch<'w> = OptionReadFetch<T>;
    type State = Option<ComponentId>;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    /// Optional always matches – missing columns yield `None`.
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        true
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, _world_tick: u32) -> Self::Fetch<'w> {
        let ptr = state
            .and_then(|id| archetype.column_index(id))
            .map(|col| archetype.column_data_ptr(col) as *const u8);
        OptionReadFetch {
            ptr,
            size: std::mem::size_of::<T>(),
            _marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        fetch
            .ptr
            .map(|p| &*(p.add(row * fetch.size) as *const T))
    }
}

unsafe impl<T: Component> ReadOnlyWorldQuery for Option<&T> {}

// -- Option<&mut T> ----------------------------------------------------------

unsafe impl<T: Component> WorldQuery for Option<&mut T> {
    type Item<'w> = Option<&'w mut T>;
    type Fetch<'w> = OptionWriteFetch<T>;
    type State = Option<ComponentId>;

    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }

    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        true
    }

    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, _world_tick: u32) -> Self::Fetch<'w> {
        let ptr = state
            .and_then(|id| archetype.column_index(id))
            .map(|col| archetype.column_data_ptr(col));
        OptionWriteFetch {
            ptr,
            size: std::mem::size_of::<T>(),
            _marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        fetch
            .ptr
            .map(|p| &mut *(p.add(row * fetch.size) as *mut T))
    }
}

// -- Entity (yields the entity handle) ---------------------------------------

unsafe impl WorldQuery for Entity {
    type Item<'w> = Entity;
    type Fetch<'w> = EntityFetch<'w>;
    type State = ();

    fn init_state(_registry: &ComponentRegistry) -> Self::State {}

    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        true
    }

    fn init_fetch<'w>(_state: &Self::State, archetype: &'w Archetype, _world_tick: u32) -> Self::Fetch<'w> {
        EntityFetch {
            entities: archetype.entities(),
        }
    }

    #[inline]
    unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        fetch.entities[row]
    }
}

unsafe impl ReadOnlyWorldQuery for Entity {}

// ---------------------------------------------------------------------------
// Tuple impls via macro
// ---------------------------------------------------------------------------

macro_rules! impl_world_query_tuple {
    ($($T:ident $idx:tt),+) => {
        unsafe impl<$($T: WorldQuery),+> WorldQuery for ($($T,)+) {
            type Item<'w> = ($($T::Item<'w>,)+);
            type Fetch<'w> = ($($T::Fetch<'w>,)+);
            type State = ($($T::State,)+);

            fn init_state(registry: &ComponentRegistry) -> Self::State {
                ($($T::init_state(registry),)+)
            }

            fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
                $($T::matches_archetype(&state.$idx, archetype))&&+
            }

            fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w> {
                ($($T::init_fetch(&state.$idx, archetype, world_tick),)+)
            }

            #[inline]
            unsafe fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
                ($($T::fetch(&mut fetch.$idx, row),)+)
            }
        }

        unsafe impl<$($T: ReadOnlyWorldQuery),+> ReadOnlyWorldQuery for ($($T,)+) {}
    };
}

impl_world_query_tuple!(A 0);
impl_world_query_tuple!(A 0, B 1);
impl_world_query_tuple!(A 0, B 1, C 2);
impl_world_query_tuple!(A 0, B 1, C 2, D 3);
impl_world_query_tuple!(A 0, B 1, C 2, D 3, E 4);
impl_world_query_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
impl_world_query_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
impl_world_query_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);

// ---------------------------------------------------------------------------
// QueryFilter – archetype-level + per-row filtering
// ---------------------------------------------------------------------------

pub trait QueryFilter {
    type State;
    type Fetch<'w>;

    fn init_state(registry: &ComponentRegistry) -> Self::State;
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool;
    fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w>;

    /// Per-row filtering. Returns `true` if this row passes the filter.
    /// For archetype-only filters (With, Without), this always returns `true`.
    fn matches_row(fetch: &Self::Fetch<'_>, row: usize) -> bool;
}

/// No filter – always matches.
impl QueryFilter for () {
    type State = ();
    type Fetch<'w> = ();
    fn init_state(_: &ComponentRegistry) -> Self::State {}
    fn matches_archetype(_: &Self::State, _: &Archetype) -> bool {
        true
    }
    fn init_fetch<'w>(_: &Self::State, _: &'w Archetype, _: u32) -> Self::Fetch<'w> {}
    #[inline]
    fn matches_row(_: &Self::Fetch<'_>, _: usize) -> bool {
        true
    }
}

/// Include only archetypes that **have** component `T`.
pub struct With<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for With<T> {
    type State = Option<ComponentId>;
    type Fetch<'w> = ();
    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.map_or(false, |id| archetype.has_component(id))
    }
    fn init_fetch<'w>(_: &Self::State, _: &'w Archetype, _: u32) -> Self::Fetch<'w> {}
    #[inline]
    fn matches_row(_: &Self::Fetch<'_>, _: usize) -> bool {
        true
    }
}

/// Exclude archetypes that have component `T`.
pub struct Without<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for Without<T> {
    type State = Option<ComponentId>;
    type Fetch<'w> = ();
    fn init_state(registry: &ComponentRegistry) -> Self::State {
        registry.lookup::<T>()
    }
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        // If T was never registered, no archetype has it → always passes.
        state.map_or(true, |id| !archetype.has_component(id))
    }
    fn init_fetch<'w>(_: &Self::State, _: &'w Archetype, _: u32) -> Self::Fetch<'w> {}
    #[inline]
    fn matches_row(_: &Self::Fetch<'_>, _: usize) -> bool {
        true
    }
}

// Tuple filters

macro_rules! impl_query_filter_tuple {
    ($($T:ident $idx:tt),+) => {
        impl<$($T: QueryFilter),+> QueryFilter for ($($T,)+) {
            type State = ($($T::State,)+);
            type Fetch<'w> = ($($T::Fetch<'w>,)+);

            fn init_state(registry: &ComponentRegistry) -> Self::State {
                ($($T::init_state(registry),)+)
            }

            fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
                $($T::matches_archetype(&state.$idx, archetype))&&+
            }

            fn init_fetch<'w>(state: &Self::State, archetype: &'w Archetype, world_tick: u32) -> Self::Fetch<'w> {
                ($($T::init_fetch(&state.$idx, archetype, world_tick),)+)
            }

            #[inline]
            fn matches_row(fetch: &Self::Fetch<'_>, row: usize) -> bool {
                $($T::matches_row(&fetch.$idx, row))&&+
            }
        }
    };
}

impl_query_filter_tuple!(A 0);
impl_query_filter_tuple!(A 0, B 1);
impl_query_filter_tuple!(A 0, B 1, C 2);
impl_query_filter_tuple!(A 0, B 1, C 2, D 3);

// ---------------------------------------------------------------------------
// QueryIter
// ---------------------------------------------------------------------------

/// Iterator over entities matching a `WorldQuery` + optional `QueryFilter`.
pub struct QueryIter<'w, Q: WorldQuery, F: QueryFilter = ()> {
    archetypes: &'w [Archetype],
    matching: Vec<usize>,
    current_match: usize,
    current_row: usize,
    current_len: usize,
    current_fetch: Option<Q::Fetch<'w>>,
    current_filter_fetch: Option<F::Fetch<'w>>,
    query_state: Q::State,
    filter_state: F::State,
    world_tick: u32,
    _filter: PhantomData<F>,
}

impl<'w, Q: WorldQuery, F: QueryFilter> QueryIter<'w, Q, F> {
    pub(crate) fn new(
        archetypes: &'w [Archetype],
        query_state: Q::State,
        filter_state: F::State,
        world_tick: u32,
    ) -> Self {
        let matching: Vec<usize> = archetypes
            .iter()
            .enumerate()
            .filter(|(_, arch)| {
                !arch.is_empty()
                    && Q::matches_archetype(&query_state, arch)
                    && F::matches_archetype(&filter_state, arch)
            })
            .map(|(i, _)| i)
            .collect();

        Self {
            archetypes,
            matching,
            current_match: 0,
            current_row: 0,
            current_len: 0,
            current_fetch: None,
            current_filter_fetch: None,
            query_state,
            filter_state,
            world_tick,
            _filter: PhantomData,
        }
    }

    /// Consume the iterator, returning the single matching item.
    ///
    /// # Panics
    /// Panics if 0 or ≥2 items match.
    pub fn single(mut self) -> Q::Item<'w> {
        let item = self.next().expect("query::single found no results");
        assert!(self.next().is_none(), "query::single found multiple results");
        item
    }
}

impl<'w, Q: WorldQuery, F: QueryFilter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_row < self.current_len {
                let row = self.current_row;
                self.current_row += 1;
                // Per-row filter check (always true for archetype-only filters).
                if F::matches_row(self.current_filter_fetch.as_ref().unwrap(), row) {
                    return Some(unsafe { Q::fetch(self.current_fetch.as_mut().unwrap(), row) });
                }
                continue;
            }

            if self.current_match >= self.matching.len() {
                return None;
            }

            let arch_idx = self.matching[self.current_match];
            self.current_match += 1;
            let archetype = &self.archetypes[arch_idx];

            self.current_fetch = Some(Q::init_fetch(&self.query_state, archetype, self.world_tick));
            self.current_filter_fetch = Some(F::init_fetch(&self.filter_state, archetype, self.world_tick));
            self.current_len = archetype.len();
            self.current_row = 0;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_in_current = self.current_len.saturating_sub(self.current_row);
        let remaining_archetypes: usize = self.matching[self.current_match..]
            .iter()
            .map(|&i| self.archetypes[i].len())
            .sum();
        let total = remaining_in_current + remaining_archetypes;
        (0, Some(total))
    }
}
