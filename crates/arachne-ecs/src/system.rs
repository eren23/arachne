use crate::commands::{CommandQueue, Commands};
use crate::event::{EventQueue, EventReader, EventWriter};
use crate::query::{QueryFilter, QueryIter, ReadOnlyWorldQuery, WorldQuery};
use crate::resource::{Res, ResMut};
use crate::world::World;
use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// System trait
// ---------------------------------------------------------------------------

pub trait System: Send + 'static {
    fn run(&mut self, world: &mut World);
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// IntoSystem – converts callables into System
// ---------------------------------------------------------------------------

pub trait IntoSystem<Marker>: Sized {
    type System: System;
    fn into_system(self) -> Self::System;
}

// ---------------------------------------------------------------------------
// SystemParam – extracting typed parameters from World
// ---------------------------------------------------------------------------

/// Trait for types that can be extracted from `&mut World` as system
/// parameters.
///
/// # Safety
/// Implementations must only access disjoint parts of World (e.g. queries
/// access archetypes, Res accesses resources).
pub unsafe trait SystemParam: Sized {
    /// Extract this parameter from the world.
    ///
    /// # Safety
    /// `world` must be a valid pointer to a `World`. The caller must
    /// ensure no aliasing violations between simultaneously extracted params.
    unsafe fn get(world: *mut World) -> Self;
}

// ---------------------------------------------------------------------------
// Query<Q, F> – system parameter for queries
// ---------------------------------------------------------------------------

/// System parameter that provides query iteration over entities.
pub struct Query<Q: WorldQuery, F: QueryFilter = ()> {
    world_ptr: *mut World,
    _marker: PhantomData<fn() -> (Q, F)>,
}

// SAFETY: Query is only alive during system execution.
unsafe impl<Q: WorldQuery, F: QueryFilter> Send for Query<Q, F> {}
unsafe impl<Q: WorldQuery, F: QueryFilter> Sync for Query<Q, F> {}

impl<Q: WorldQuery, F: QueryFilter> Query<Q, F> {
    /// Iterate matching entities (read-only).
    pub fn iter(&self) -> QueryIter<'_, Q, F>
    where
        Q: ReadOnlyWorldQuery,
    {
        unsafe {
            let world = &*self.world_ptr;
            let query_state = Q::init_state(&world.registry);
            let filter_state = F::init_state(&world.registry);
            QueryIter::new(&world.archetypes, query_state, filter_state, world.tick)
        }
    }

    /// Iterate matching entities (mutable access).
    pub fn iter_mut(&mut self) -> QueryIter<'_, Q, F> {
        unsafe {
            let world = &*self.world_ptr;
            let query_state = Q::init_state(&world.registry);
            let filter_state = F::init_state(&world.registry);
            QueryIter::new(&world.archetypes, query_state, filter_state, world.tick)
        }
    }
}

impl<Q: ReadOnlyWorldQuery, F: QueryFilter> IntoIterator for Query<Q, F> {
    type Item = Q::Item<'static>;
    type IntoIter = QueryIter<'static, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        unsafe {
            let world = &*self.world_ptr;
            let query_state = Q::init_state(&world.registry);
            let filter_state = F::init_state(&world.registry);
            // SAFETY: The query outlives its usage within the system call.
            // We transmute the lifetime to 'static because the query is only
            // used during system execution while World is alive.
            let iter: QueryIter<'_, Q, F> = QueryIter::new(
                &world.archetypes,
                query_state,
                filter_state,
                world.tick,
            );
            std::mem::transmute(iter)
        }
    }
}

unsafe impl<Q: WorldQuery, F: QueryFilter> SystemParam for Query<Q, F> {
    unsafe fn get(world: *mut World) -> Self {
        Query {
            world_ptr: world,
            _marker: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// SystemParam impls for resource types
// ---------------------------------------------------------------------------

unsafe impl<T: 'static + Send + Sync> SystemParam for Res<T> {
    unsafe fn get(world: *mut World) -> Self {
        let world_ref = &*world;
        let ptr = world_ref.resources.get::<T>() as *const T;
        Res::new(ptr)
    }
}

unsafe impl<T: 'static + Send + Sync> SystemParam for ResMut<T> {
    unsafe fn get(world: *mut World) -> Self {
        let world_mut = &mut *world;
        let ptr = world_mut.resources.get_mut::<T>() as *mut T;
        ResMut::new(ptr)
    }
}

unsafe impl<T: 'static + Send + Sync> SystemParam for Option<Res<T>> {
    unsafe fn get(world: *mut World) -> Self {
        let world_ref = &*world;
        world_ref
            .resources
            .try_get::<T>()
            .map(|val| Res::new(val as *const T))
    }
}

unsafe impl<T: 'static + Send + Sync> SystemParam for Option<ResMut<T>> {
    unsafe fn get(world: *mut World) -> Self {
        let world_mut = &mut *world;
        world_mut
            .resources
            .try_get_mut::<T>()
            .map(|val| ResMut::new(val as *mut T))
    }
}

// ---------------------------------------------------------------------------
// SystemParam impls for event types
// ---------------------------------------------------------------------------

unsafe impl<T: 'static + Send + Sync> SystemParam for EventReader<T> {
    unsafe fn get(world: *mut World) -> Self {
        let world_ref = &*world;
        let queue = world_ref.events.get::<T>() as *const EventQueue<T>;
        EventReader::new(queue)
    }
}

unsafe impl<T: 'static + Send + Sync> SystemParam for EventWriter<T> {
    unsafe fn get(world: *mut World) -> Self {
        let world_mut = &mut *world;
        let queue = world_mut.events.get_mut::<T>() as *mut EventQueue<T>;
        EventWriter::new(queue)
    }
}

// ---------------------------------------------------------------------------
// SystemParam impl for Commands
// ---------------------------------------------------------------------------

unsafe impl SystemParam for Commands {
    unsafe fn get(world: *mut World) -> Self {
        Commands::new(&mut (*world).command_queue as *mut CommandQueue)
    }
}

// ---------------------------------------------------------------------------
// FunctionSystem + IntoSystem macro for 0..8 params
// ---------------------------------------------------------------------------

/// A system backed by a Rust function/closure.
pub struct FunctionSystem<F, Marker> {
    func: F,
    name: String,
    _marker: PhantomData<fn() -> Marker>,
}

impl<F, Marker> FunctionSystem<F, Marker> {
    pub fn new(func: F, name: String) -> Self {
        Self {
            func,
            name,
            _marker: PhantomData,
        }
    }
}

// -- 0 params ----------------------------------------------------------------

impl<F> System for FunctionSystem<F, ()>
where
    F: FnMut() + Send + 'static,
{
    fn run(&mut self, _world: &mut World) {
        (self.func)();
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl<F> IntoSystem<()> for F
where
    F: FnMut() + Send + 'static,
{
    type System = FunctionSystem<F, ()>;
    fn into_system(self) -> Self::System {
        FunctionSystem::new(self, std::any::type_name::<F>().to_string())
    }
}

// -- Macro for 1..8 params ---------------------------------------------------

macro_rules! impl_function_system {
    ($($P:ident $idx:tt),+) => {
        impl<F, $($P: SystemParam + 'static),+> System for FunctionSystem<F, ($($P,)+)>
        where
            F: FnMut($($P),+) + Send + 'static,
        {
            fn run(&mut self, world: &mut World) {
                let world_ptr = world as *mut World;
                $(
                    #[allow(non_snake_case)]
                    let $P = unsafe { <$P as SystemParam>::get(world_ptr) };
                )+
                (self.func)($($P),+);
            }

            fn name(&self) -> &str {
                &self.name
            }
        }

        impl<F, $($P: SystemParam + 'static),+> IntoSystem<($($P,)+)> for F
        where
            F: FnMut($($P),+) + Send + 'static,
        {
            type System = FunctionSystem<F, ($($P,)+)>;
            fn into_system(self) -> Self::System {
                FunctionSystem::new(self, std::any::type_name::<F>().to_string())
            }
        }
    };
}

impl_function_system!(P0 0);
impl_function_system!(P0 0, P1 1);
impl_function_system!(P0 0, P1 1, P2 2);
impl_function_system!(P0 0, P1 1, P2 2, P3 3);
impl_function_system!(P0 0, P1 1, P2 2, P3 3, P4 4);
impl_function_system!(P0 0, P1 1, P2 2, P3 3, P4 4, P5 5);
impl_function_system!(P0 0, P1 1, P2 2, P3 3, P4 4, P5 5, P6 6);
impl_function_system!(P0 0, P1 1, P2 2, P3 3, P4 4, P5 5, P6 6, P7 7);
