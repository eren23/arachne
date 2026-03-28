use std::any::{Any, TypeId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// EventQueue<T> – double-buffered event storage
// ---------------------------------------------------------------------------

/// Double-buffered event storage. Writers push to the active buffer; after
/// `swap()`, readers see those events in the read buffer while the active
/// buffer is cleared for new writes.
pub struct EventQueue<T: 'static + Send + Sync> {
    /// Two buffers: one for writing (active), one for reading.
    buffers: [Vec<T>; 2],
    /// Index of the write buffer (0 or 1). The read buffer is `1 - write_idx`.
    write_idx: usize,
}

impl<T: 'static + Send + Sync> EventQueue<T> {
    pub fn new() -> Self {
        Self {
            buffers: [Vec::new(), Vec::new()],
            write_idx: 0,
        }
    }

    /// Push an event into the current write buffer.
    #[inline]
    pub fn send(&mut self, event: T) {
        self.buffers[self.write_idx].push(event);
    }

    /// Swap buffers: the write buffer becomes the read buffer, and the old
    /// read buffer is cleared to become the new write buffer.
    pub fn swap(&mut self) {
        self.write_idx = 1 - self.write_idx;
        self.buffers[self.write_idx].clear();
    }

    /// Iterate events from the read buffer (events written last frame).
    #[inline]
    pub fn read(&self) -> &[T] {
        &self.buffers[1 - self.write_idx]
    }

    /// Number of readable events.
    #[inline]
    pub fn len(&self) -> usize {
        self.buffers[1 - self.write_idx].len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffers[1 - self.write_idx].is_empty()
    }
}

impl<T: 'static + Send + Sync> Default for EventQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EventReader<T> / EventWriter<T> – system parameter types
// ---------------------------------------------------------------------------

/// Read-only view of events from the previous frame.
pub struct EventReader<T: 'static + Send + Sync> {
    ptr: *const EventQueue<T>,
}

unsafe impl<T: 'static + Send + Sync> Send for EventReader<T> {}
unsafe impl<T: 'static + Send + Sync> Sync for EventReader<T> {}

impl<T: 'static + Send + Sync> EventReader<T> {
    pub(crate) fn new(ptr: *const EventQueue<T>) -> Self {
        Self { ptr }
    }

    /// Iterate readable events.
    #[inline]
    pub fn read(&self) -> &[T] {
        unsafe { (*self.ptr).read() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        unsafe { (*self.ptr).len() }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        unsafe { (*self.ptr).is_empty() }
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.read().iter()
    }
}

/// Write-only view for pushing events into the current frame's write buffer.
pub struct EventWriter<T: 'static + Send + Sync> {
    ptr: *mut EventQueue<T>,
}

unsafe impl<T: 'static + Send + Sync> Send for EventWriter<T> {}
unsafe impl<T: 'static + Send + Sync> Sync for EventWriter<T> {}

impl<T: 'static + Send + Sync> EventWriter<T> {
    pub(crate) fn new(ptr: *mut EventQueue<T>) -> Self {
        Self { ptr }
    }

    #[inline]
    pub fn send(&mut self, event: T) {
        unsafe { (*self.ptr).send(event) }
    }
}

// ---------------------------------------------------------------------------
// EventStorage – type-erased collection of all event queues
// ---------------------------------------------------------------------------

pub(crate) trait AnyEventQueue: Send + Sync {
    fn swap(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static + Send + Sync> AnyEventQueue for EventQueue<T> {
    fn swap(&mut self) {
        EventQueue::swap(self);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct EventStorage {
    queues: HashMap<TypeId, Box<dyn AnyEventQueue>>,
}

impl EventStorage {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }

    /// Register an event type. No-op if already registered.
    pub fn register<T: 'static + Send + Sync>(&mut self) {
        self.queues
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(EventQueue::<T>::new()));
    }

    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        self.queues.contains_key(&TypeId::of::<T>())
    }

    pub fn get<T: 'static + Send + Sync>(&self) -> &EventQueue<T> {
        self.queues
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "event queue for `{}` not registered — call world.add_event::<T>() first",
                    std::any::type_name::<T>()
                )
            })
            .as_any()
            .downcast_ref::<EventQueue<T>>()
            .unwrap()
    }

    pub fn get_mut<T: 'static + Send + Sync>(&mut self) -> &mut EventQueue<T> {
        self.queues
            .get_mut(&TypeId::of::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "event queue for `{}` not registered — call world.add_event::<T>() first",
                    std::any::type_name::<T>()
                )
            })
            .as_any_mut()
            .downcast_mut::<EventQueue<T>>()
            .unwrap()
    }

    /// Swap all registered event queues (call at frame boundary).
    pub fn swap_all(&mut self) {
        for queue in self.queues.values_mut() {
            queue.swap();
        }
    }
}

impl Default for EventStorage {
    fn default() -> Self {
        Self::new()
    }
}
