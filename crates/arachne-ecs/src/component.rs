use std::alloc::Layout;
use std::any::TypeId;
use std::collections::HashMap;

/// Marker trait for types that can be stored as ECS components.
pub trait Component: 'static + Send + Sync {}

/// Blanket impl: any `'static + Send + Sync` type is a Component.
impl<T: 'static + Send + Sync> Component for T {}

/// Dense index identifying a registered component type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(pub(crate) u32);

/// Runtime metadata for a registered component type.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub id: ComponentId,
    pub type_id: TypeId,
    pub layout: Layout,
    /// Function pointer that runs `drop_in_place` for this component type.
    /// `None` for types that do not need drop (e.g. `Copy` types with no `Drop`).
    pub drop_fn: Option<unsafe fn(*mut u8)>,
    pub name: &'static str,
}

/// Registry that maps Rust types to dense `ComponentId`s.
pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
    type_map: HashMap<TypeId, ComponentId>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            infos: Vec::new(),
            type_map: HashMap::new(),
        }
    }

    /// Register a component type (or return its existing id).
    pub fn get_or_register<T: Component>(&mut self) -> ComponentId {
        let type_id = TypeId::of::<T>();
        if let Some(&id) = self.type_map.get(&type_id) {
            return id;
        }
        let id = ComponentId(self.infos.len() as u32);
        let drop_fn: Option<unsafe fn(*mut u8)> = if std::mem::needs_drop::<T>() {
            Some(|ptr: *mut u8| unsafe {
                std::ptr::drop_in_place(ptr as *mut T);
            })
        } else {
            None
        };
        self.infos.push(ComponentInfo {
            id,
            type_id,
            layout: Layout::new::<T>(),
            drop_fn,
            name: std::any::type_name::<T>(),
        });
        self.type_map.insert(type_id, id);
        id
    }

    /// Look up an already-registered type. Returns `None` if never registered.
    #[inline]
    pub fn lookup<T: Component>(&self) -> Option<ComponentId> {
        self.type_map.get(&TypeId::of::<T>()).copied()
    }

    #[inline]
    pub fn get_info(&self, id: ComponentId) -> &ComponentInfo {
        &self.infos[id.0 as usize]
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.infos.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.infos.is_empty()
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
