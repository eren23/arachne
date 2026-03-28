use std::any::{Any, TypeId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ResourceMap – global singleton storage keyed by type
// ---------------------------------------------------------------------------

pub struct ResourceMap {
    resources: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ResourceMap {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }

    pub fn insert<T: 'static + Send + Sync>(&mut self, value: T) {
        self.resources.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: 'static + Send + Sync>(&self) -> &T {
        self.resources
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "resource `{}` not found — did you forget to insert it?",
                    std::any::type_name::<T>()
                )
            })
            .downcast_ref::<T>()
            .unwrap()
    }

    pub fn get_mut<T: 'static + Send + Sync>(&mut self) -> &mut T {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "resource `{}` not found — did you forget to insert it?",
                    std::any::type_name::<T>()
                )
            })
            .downcast_mut::<T>()
            .unwrap()
    }

    pub fn try_get<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    pub fn try_get_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }

    pub fn remove<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .map(|boxed| *boxed.downcast::<T>().unwrap())
    }

    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }
}

impl Default for ResourceMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Res<T> / ResMut<T> – system parameter wrappers (no lifetime, raw-pointer)
// ---------------------------------------------------------------------------

/// Immutable resource access for systems. Created by `SystemParam::get`.
pub struct Res<T: 'static + Send + Sync> {
    ptr: *const T,
}

// SAFETY: Res is only alive during system execution with exclusive World access.
unsafe impl<T: 'static + Send + Sync> Send for Res<T> {}
unsafe impl<T: 'static + Send + Sync> Sync for Res<T> {}

impl<T: 'static + Send + Sync> Res<T> {
    pub(crate) fn new(ptr: *const T) -> Self {
        Self { ptr }
    }
}

impl<T: 'static + Send + Sync> std::ops::Deref for Res<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

/// Mutable resource access for systems. Created by `SystemParam::get`.
pub struct ResMut<T: 'static + Send + Sync> {
    ptr: *mut T,
}

unsafe impl<T: 'static + Send + Sync> Send for ResMut<T> {}
unsafe impl<T: 'static + Send + Sync> Sync for ResMut<T> {}

impl<T: 'static + Send + Sync> ResMut<T> {
    pub(crate) fn new(ptr: *mut T) -> Self {
        Self { ptr }
    }
}

impl<T: 'static + Send + Sync> std::ops::Deref for ResMut<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: 'static + Send + Sync> std::ops::DerefMut for ResMut<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}
