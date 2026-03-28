use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, ComponentRegistry};

// ---------------------------------------------------------------------------
// Bundle – describes a set of components that can be spawned together
// ---------------------------------------------------------------------------

/// A set of components that can be inserted as a unit.
///
/// # Safety
/// `component_ids` must return a sorted, deduplicated list.
/// `write_to_archetype` must write exactly one value per column.
pub unsafe trait Bundle: 'static {
    fn component_ids(registry: &mut ComponentRegistry) -> Vec<ComponentId>;

    /// Write component data into the archetype's columns.
    ///
    /// # Safety
    /// The archetype must have columns matching `component_ids`, and the caller
    /// must call `push_entity` afterwards.
    unsafe fn write_to_archetype(self, archetype: &mut Archetype, registry: &ComponentRegistry, tick: u32);
}

// ---------------------------------------------------------------------------
// Bundle for tuples (up to 12 elements)
// ---------------------------------------------------------------------------

macro_rules! impl_bundle_tuple {
    ($($T:ident $idx:tt),+) => {
        unsafe impl<$($T: Component),+> Bundle for ($($T,)+) {
            fn component_ids(registry: &mut ComponentRegistry) -> Vec<ComponentId> {
                let mut ids = vec![$(registry.get_or_register::<$T>()),+];
                ids.sort();
                let orig_len = ids.len();
                ids.dedup();
                assert_eq!(ids.len(), orig_len, "duplicate component types in bundle");
                ids
            }

            unsafe fn write_to_archetype(self, archetype: &mut Archetype, registry: &ComponentRegistry, tick: u32) {
                $(
                    let id = registry.lookup::<$T>().unwrap();
                    let col = archetype.column_index(id).unwrap();
                    archetype.push_component_data(col, std::ptr::addr_of!(self.$idx) as *const u8, tick);
                )+
                std::mem::forget(self);
            }
        }
    };
}

impl_bundle_tuple!(A 0);
impl_bundle_tuple!(A 0, B 1);
impl_bundle_tuple!(A 0, B 1, C 2);
impl_bundle_tuple!(A 0, B 1, C 2, D 3);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10);
impl_bundle_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11);
