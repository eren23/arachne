use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Unique identifier for an asset, derived from its path hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HandleId(pub u64);

impl HandleId {
    pub fn from_path(path: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        HandleId(hasher.finish())
    }
}

/// Shared reference count tracking how many strong handles exist.
/// The atomic counter tracks only user-held strong handles.
/// Server/cache hold Arc clones but never increment the counter.
pub(crate) type RefCount = Arc<AtomicU32>;

pub(crate) fn new_ref_count() -> RefCount {
    Arc::new(AtomicU32::new(0))
}

/// Returns the number of strong handles currently held by users.
#[cfg(test)]
pub(crate) fn strong_count(rc: &RefCount) -> u32 {
    rc.load(Ordering::Relaxed)
}

/// Typed reference to an asset. Can be strong (ref-counted, keeps asset alive)
/// or weak (just an ID, does not prevent eviction).
pub struct Handle<T: 'static> {
    id: HandleId,
    ref_count: Option<RefCount>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: 'static> Handle<T> {
    /// Create a strong handle. Increments the reference count.
    pub(crate) fn strong(id: HandleId, ref_count: RefCount) -> Self {
        ref_count.fetch_add(1, Ordering::Relaxed);
        Handle {
            id,
            ref_count: Some(ref_count),
            _marker: PhantomData,
        }
    }

    /// Create a weak handle. Does not affect reference count.
    pub fn weak(id: HandleId) -> Self {
        Handle {
            id,
            ref_count: None,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> HandleId {
        self.id
    }

    pub fn is_strong(&self) -> bool {
        self.ref_count.is_some()
    }

    pub fn is_weak(&self) -> bool {
        self.ref_count.is_none()
    }

    /// Downgrade a strong handle to a weak handle.
    pub fn downgrade(&self) -> Handle<T> {
        Handle::weak(self.id)
    }

    /// Get the current strong reference count (0 for weak handles).
    pub fn ref_count(&self) -> u32 {
        match &self.ref_count {
            Some(rc) => rc.load(Ordering::Relaxed),
            None => 0,
        }
    }
}

impl<T: 'static> Clone for Handle<T> {
    fn clone(&self) -> Self {
        if let Some(rc) = &self.ref_count {
            rc.fetch_add(1, Ordering::Relaxed);
        }
        Handle {
            id: self.id,
            ref_count: self.ref_count.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> Drop for Handle<T> {
    fn drop(&mut self) {
        if let Some(rc) = &self.ref_count {
            rc.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl<T: 'static> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.is_strong() { "Strong" } else { "Weak" };
        write!(f, "Handle<{}>({}, {:?})", std::any::type_name::<T>(), kind, self.id)
    }
}

// Handle is Send + Sync regardless of T because PhantomData<fn() -> T> is.
// Arc<AtomicU32> is Send + Sync.
unsafe impl<T: 'static> Send for Handle<T> {}
unsafe impl<T: 'static> Sync for Handle<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAsset;

    #[test]
    fn handle_id_from_path_deterministic() {
        let a = HandleId::from_path("textures/player.png");
        let b = HandleId::from_path("textures/player.png");
        assert_eq!(a, b);

        let c = HandleId::from_path("textures/enemy.png");
        assert_ne!(a, c);
    }

    #[test]
    fn strong_handle_ref_count() {
        let rc = new_ref_count();
        assert_eq!(strong_count(&rc), 0);

        let h1: Handle<DummyAsset> = Handle::strong(HandleId(1), rc.clone());
        assert_eq!(strong_count(&rc), 1);
        assert!(h1.is_strong());
        assert_eq!(h1.ref_count(), 1);

        let h2 = h1.clone();
        assert_eq!(strong_count(&rc), 2);
        assert_eq!(h2.ref_count(), 2);

        drop(h2);
        assert_eq!(strong_count(&rc), 1);

        drop(h1);
        assert_eq!(strong_count(&rc), 0);
    }

    #[test]
    fn weak_handle_does_not_affect_count() {
        let rc = new_ref_count();
        let h1: Handle<DummyAsset> = Handle::strong(HandleId(1), rc.clone());
        assert_eq!(strong_count(&rc), 1);

        let weak = h1.downgrade();
        assert!(weak.is_weak());
        assert_eq!(weak.ref_count(), 0);
        assert_eq!(strong_count(&rc), 1); // unchanged

        let weak2 = weak.clone();
        assert_eq!(strong_count(&rc), 1); // still unchanged

        drop(weak);
        drop(weak2);
        assert_eq!(strong_count(&rc), 1); // still 1

        drop(h1);
        assert_eq!(strong_count(&rc), 0);
    }

    #[test]
    fn multiple_strong_handles_track_correctly() {
        let rc = new_ref_count();
        let id = HandleId(42);

        let h1: Handle<DummyAsset> = Handle::strong(id, rc.clone());
        let h2: Handle<DummyAsset> = Handle::strong(id, rc.clone());
        let h3 = h1.clone();

        assert_eq!(strong_count(&rc), 3);

        drop(h1);
        assert_eq!(strong_count(&rc), 2);

        drop(h3);
        assert_eq!(strong_count(&rc), 1);

        drop(h2);
        assert_eq!(strong_count(&rc), 0);
    }

    #[test]
    fn handle_preserves_id() {
        let id = HandleId::from_path("models/cube.obj");
        let rc = new_ref_count();
        let h: Handle<DummyAsset> = Handle::strong(id, rc.clone());
        assert_eq!(h.id(), id);

        let weak: Handle<DummyAsset> = Handle::weak(id);
        assert_eq!(weak.id(), id);
    }
}
