use crate::component::{ComponentId, ComponentRegistry};
use crate::entity::Entity;
use std::alloc::{self, Layout};
use std::collections::HashMap;
use std::ptr::NonNull;

// ---------------------------------------------------------------------------
// BlobVec – type-erased, alignment-aware column storage
// ---------------------------------------------------------------------------

/// A `Vec<u8>`-like container that stores items of a single (erased) type with
/// correct size, alignment, and drop semantics.
pub(crate) struct BlobVec {
    item_layout: Layout,
    /// Always aligned to `item_layout.align()`.  `NonNull::dangling()` when
    /// capacity is 0 or item size is 0 (ZST).
    data: NonNull<u8>,
    capacity: usize,
    len: usize,
    drop_fn: Option<unsafe fn(*mut u8)>,
}

// SAFETY: The data pointer is exclusively owned; Send/Sync are fine as long as
// the stored type is Send+Sync (guaranteed by Component bound at usage sites).
unsafe impl Send for BlobVec {}
unsafe impl Sync for BlobVec {}

impl BlobVec {
    /// Create a new, empty `BlobVec` for items described by `item_layout`.
    pub fn new(item_layout: Layout, drop_fn: Option<unsafe fn(*mut u8)>) -> Self {
        Self {
            item_layout,
            data: NonNull::dangling(),
            capacity: 0,
            len: 0,
            drop_fn,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Raw data pointer. Callers must ensure aliasing rules.
    #[inline]
    pub fn data_ptr(&self) -> *mut u8 {
        self.data.as_ptr()
    }

    #[inline]
    fn item_size(&self) -> usize {
        self.item_layout.size()
    }

    /// Pointer to element at `index` (no bounds check in release).
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> *const u8 {
        debug_assert!(index < self.len);
        self.data.as_ptr().add(index * self.item_size())
    }

    /// Mutable pointer to element at `index`.
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> *mut u8 {
        debug_assert!(index < self.len);
        self.data.as_ptr().add(index * self.item_size())
    }

    /// Append an item by copying `item_size` bytes from `src`.
    ///
    /// # Safety
    /// `src` must point to a valid instance of the stored type.
    pub unsafe fn push(&mut self, src: *const u8) {
        if self.item_size() == 0 {
            self.len += 1;
            return;
        }
        self.reserve(1);
        let dst = self.data.as_ptr().add(self.len * self.item_size());
        std::ptr::copy_nonoverlapping(src, dst, self.item_size());
        self.len += 1;
    }

    /// Swap-remove element at `index` **and call its drop function**.
    ///
    /// # Safety
    /// `index` must be < `self.len`.
    pub unsafe fn swap_remove_drop(&mut self, index: usize) {
        debug_assert!(index < self.len);
        let size = self.item_size();
        if size == 0 {
            self.len -= 1;
            return;
        }
        let ptr = self.data.as_ptr().add(index * size);
        if let Some(drop_fn) = self.drop_fn {
            (drop_fn)(ptr);
        }
        let last = self.len - 1;
        if index != last {
            let src = self.data.as_ptr().add(last * size);
            std::ptr::copy_nonoverlapping(src, ptr, size);
        }
        self.len -= 1;
    }

    /// Swap-remove element at `index` **without** calling drop (caller moved
    /// the data elsewhere).
    ///
    /// # Safety
    /// `index` must be < `self.len`. Caller is responsible for the removed
    /// element's resources.
    pub unsafe fn swap_remove_forget(&mut self, index: usize) {
        debug_assert!(index < self.len);
        let size = self.item_size();
        if size == 0 {
            self.len -= 1;
            return;
        }
        let last = self.len - 1;
        if index != last {
            let src = self.data.as_ptr().add(last * size);
            let dst = self.data.as_ptr().add(index * size);
            std::ptr::copy_nonoverlapping(src, dst, size);
        }
        self.len -= 1;
    }

    /// Ensure room for `additional` more items.
    fn reserve(&mut self, additional: usize) {
        let required = self.len + additional;
        if required <= self.capacity {
            return;
        }
        let new_cap = required.max(self.capacity * 2).max(8);
        self.grow_to(new_cap);
    }

    fn grow_to(&mut self, new_cap: usize) {
        let size = self.item_size();
        if size == 0 {
            self.capacity = usize::MAX;
            return;
        }
        let new_layout =
            Layout::from_size_align(size * new_cap, self.item_layout.align()).unwrap();
        let new_ptr = if self.capacity == 0 {
            // Fresh allocation.
            unsafe { alloc::alloc(new_layout) }
        } else {
            let old_layout =
                Layout::from_size_align(size * self.capacity, self.item_layout.align()).unwrap();
            unsafe { alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size()) }
        };
        self.data = NonNull::new(new_ptr).expect("allocation failed");
        self.capacity = new_cap;
    }

    /// Drop every element and reset length to 0 (does **not** free the buffer).
    pub fn clear(&mut self) {
        if let Some(drop_fn) = self.drop_fn {
            let size = self.item_size();
            for i in 0..self.len {
                unsafe {
                    let ptr = self.data.as_ptr().add(i * size);
                    (drop_fn)(ptr);
                }
            }
        }
        self.len = 0;
    }
}

impl Drop for BlobVec {
    fn drop(&mut self) {
        self.clear();
        let size = self.item_size();
        if size > 0 && self.capacity > 0 {
            let layout =
                Layout::from_size_align(size * self.capacity, self.item_layout.align()).unwrap();
            unsafe {
                alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Archetype
// ---------------------------------------------------------------------------

/// Identifies an archetype within the `World`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub(crate) usize);

/// A location of an entity inside an archetype.
#[derive(Debug, Clone, Copy)]
pub struct EntityLocation {
    pub archetype_id: ArchetypeId,
    pub row: usize,
}

impl EntityLocation {
    pub const INVALID: Self = Self {
        archetype_id: ArchetypeId(usize::MAX),
        row: usize::MAX,
    };
}

/// Column-oriented storage for entities that share the same component set.
pub struct Archetype {
    id: ArchetypeId,
    /// Sorted component IDs that define this archetype.
    component_ids: Vec<ComponentId>,
    /// Parallel to `component_ids` – one `BlobVec` per component type.
    columns: Vec<BlobVec>,
    /// Row → Entity mapping.
    entities: Vec<Entity>,
    /// Cached archetype transitions: "add component C → target archetype".
    add_edges: HashMap<ComponentId, ArchetypeId>,
    /// Cached archetype transitions: "remove component C → target archetype".
    remove_edges: HashMap<ComponentId, ArchetypeId>,
    /// Per-column, per-row: tick when component was last changed/written.
    change_ticks: Vec<Vec<u32>>,
    /// Per-column, per-row: tick when component was added to this entity.
    added_ticks: Vec<Vec<u32>>,
}

impl Archetype {
    /// Create an empty archetype for a given sorted set of component IDs.
    pub fn new(id: ArchetypeId, component_ids: Vec<ComponentId>, registry: &ComponentRegistry) -> Self {
        let num_cols = component_ids.len();
        let columns = component_ids
            .iter()
            .map(|&cid| {
                let info = registry.get_info(cid);
                BlobVec::new(info.layout, info.drop_fn)
            })
            .collect();
        Self {
            id,
            component_ids,
            columns,
            entities: Vec::new(),
            add_edges: HashMap::new(),
            remove_edges: HashMap::new(),
            change_ticks: (0..num_cols).map(|_| Vec::new()).collect(),
            added_ticks: (0..num_cols).map(|_| Vec::new()).collect(),
        }
    }

    // -- Accessors -----------------------------------------------------------

    #[inline]
    pub fn id(&self) -> ArchetypeId {
        self.id
    }

    #[inline]
    pub fn component_ids(&self) -> &[ComponentId] {
        &self.component_ids
    }

    #[inline]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Binary-search for the column index of `id`.
    #[inline]
    pub fn column_index(&self, id: ComponentId) -> Option<usize> {
        self.component_ids.binary_search(&id).ok()
    }

    #[inline]
    pub fn has_component(&self, id: ComponentId) -> bool {
        self.column_index(id).is_some()
    }

    /// Raw data pointer for column `col`.
    #[inline]
    pub fn column_data_ptr(&self, col: usize) -> *mut u8 {
        self.columns[col].data_ptr()
    }

    /// Item layout for column `col`.
    #[inline]
    pub fn column_item_size(&self, col: usize) -> usize {
        self.columns[col].item_size()
    }

    // -- Edges ---------------------------------------------------------------

    pub fn add_edge(&self, component: ComponentId) -> Option<ArchetypeId> {
        self.add_edges.get(&component).copied()
    }

    pub fn remove_edge(&self, component: ComponentId) -> Option<ArchetypeId> {
        self.remove_edges.get(&component).copied()
    }

    pub fn set_add_edge(&mut self, component: ComponentId, target: ArchetypeId) {
        self.add_edges.insert(component, target);
    }

    pub fn set_remove_edge(&mut self, component: ComponentId, target: ArchetypeId) {
        self.remove_edges.insert(component, target);
    }

    // -- Mutation -------------------------------------------------------------

    /// Push raw component data into column `col` with change-detection tick.
    ///
    /// # Safety
    /// * `data` must point to a valid instance of the column's component type.
    /// * Caller must push to **every** column before calling `push_entity`.
    #[inline]
    pub unsafe fn push_component_data(&mut self, col: usize, data: *const u8, tick: u32) {
        self.columns[col].push(data);
        self.change_ticks[col].push(tick);
        self.added_ticks[col].push(tick);
    }

    /// Register the entity for the most-recently-pushed row.
    #[inline]
    pub fn push_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    /// Remove entity at `row`, dropping all its component data.
    /// Returns the entity that was swap-moved into `row` (if any).
    pub fn remove_entity_drop(&mut self, row: usize) -> Option<Entity> {
        for (i, col) in self.columns.iter_mut().enumerate() {
            unsafe {
                col.swap_remove_drop(row);
            }
            self.change_ticks[i].swap_remove(row);
            self.added_ticks[i].swap_remove(row);
        }
        self.swap_remove_entity_slot(row)
    }

    /// Remove entity at `row` **without** dropping component data (the caller
    /// already moved the bytes elsewhere).
    ///
    /// # Safety
    /// Caller must have copied every column's data for `row` before calling.
    pub unsafe fn remove_entity_forget(&mut self, row: usize) -> Option<Entity> {
        for (i, col) in self.columns.iter_mut().enumerate() {
            col.swap_remove_forget(row);
            self.change_ticks[i].swap_remove(row);
            self.added_ticks[i].swap_remove(row);
        }
        self.swap_remove_entity_slot(row)
    }

    /// Overwrite component data at `(col, row)`.
    ///
    /// # Safety
    /// Drops the old value, then writes `data`.
    pub unsafe fn replace_component(&mut self, col: usize, row: usize, data: *const u8, tick: u32) {
        let size = self.columns[col].item_size();
        let ptr = self.columns[col].get_unchecked_mut(row);
        if let Some(drop_fn) = self.columns[col].drop_fn {
            (drop_fn)(ptr);
        }
        std::ptr::copy_nonoverlapping(data, ptr, size);
        self.change_ticks[col][row] = tick;
    }

    // -- Tick accessors (change detection) ------------------------------------

    /// Get the change tick for a component at a specific row.
    #[inline]
    pub fn change_tick(&self, col: usize, row: usize) -> u32 {
        self.change_ticks[col][row]
    }

    /// Get the added tick for a component at a specific row.
    #[inline]
    pub fn added_tick(&self, col: usize, row: usize) -> u32 {
        self.added_ticks[col][row]
    }

    /// Slice of change ticks for a column.
    #[inline]
    pub fn change_ticks_slice(&self, col: usize) -> &[u32] {
        &self.change_ticks[col]
    }

    /// Slice of added ticks for a column.
    #[inline]
    pub fn added_ticks_slice(&self, col: usize) -> &[u32] {
        &self.added_ticks[col]
    }

    /// Set the change tick for a specific component at a specific row.
    #[inline]
    pub fn set_change_tick(&mut self, col: usize, row: usize, tick: u32) {
        self.change_ticks[col][row] = tick;
    }

    /// Push pre-computed tick values for a column (used during archetype migration).
    #[inline]
    pub fn push_ticks(&mut self, col: usize, change_tick: u32, added_tick: u32) {
        self.change_ticks[col].push(change_tick);
        self.added_ticks[col].push(added_tick);
    }

    /// Internal: swap-remove from the entity Vec, return the swapped entity.
    fn swap_remove_entity_slot(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len() - 1;
        let swapped = if row != last {
            self.entities.swap(row, last);
            Some(self.entities[row]) // was the last, now at `row`
        } else {
            None
        };
        self.entities.pop();
        swapped
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Obtain mutable references to two distinct archetype slots.
///
/// # Panics
/// Panics if `a == b`.
pub(crate) fn get_two_mut(
    archetypes: &mut [Archetype],
    a: usize,
    b: usize,
) -> (&mut Archetype, &mut Archetype) {
    assert_ne!(a, b, "cannot borrow same archetype twice");
    if a < b {
        let (left, right) = archetypes.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = archetypes.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Layout;

    #[test]
    fn blobvec_push_and_read() {
        let layout = Layout::new::<u64>();
        let mut bv = BlobVec::new(layout, None);
        for i in 0u64..100 {
            unsafe { bv.push(&i as *const u64 as *const u8) };
        }
        assert_eq!(bv.len(), 100);
        for i in 0u64..100 {
            let val = unsafe { *(bv.get_unchecked(i as usize) as *const u64) };
            assert_eq!(val, i);
        }
    }

    #[test]
    fn blobvec_swap_remove() {
        let layout = Layout::new::<u32>();
        let mut bv = BlobVec::new(layout, None);
        for i in 0u32..5 {
            unsafe { bv.push(&i as *const u32 as *const u8) };
        }
        // Remove index 1 (value 1). Last (value 4) should move to index 1.
        unsafe { bv.swap_remove_forget(1) };
        assert_eq!(bv.len(), 4);
        let val_at_1 = unsafe { *(bv.get_unchecked(1) as *const u32) };
        assert_eq!(val_at_1, 4);
    }

    #[test]
    fn blobvec_drops_elements() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

        #[repr(C)]
        struct Dropper(u32);
        impl Drop for Dropper {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROP_COUNT.store(0, Ordering::Relaxed);
        {
            let layout = Layout::new::<Dropper>();
            let drop_fn: unsafe fn(*mut u8) =
                |ptr| unsafe { std::ptr::drop_in_place(ptr as *mut Dropper) };
            let mut bv = BlobVec::new(layout, Some(drop_fn));
            for i in 0..10u32 {
                let d = Dropper(i);
                unsafe { bv.push(&d as *const Dropper as *const u8) };
                std::mem::forget(d);
            }
            assert_eq!(bv.len(), 10);
            // BlobVec drop should call drop_fn for each element.
        }
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn blobvec_alignment() {
        // Ensure 16-byte-aligned data stays aligned.
        #[repr(align(16))]
        #[derive(Clone, Copy, PartialEq, Debug)]
        struct Aligned16(u128);

        let layout = Layout::new::<Aligned16>();
        assert_eq!(layout.align(), 16);
        let mut bv = BlobVec::new(layout, None);
        for i in 0..64u128 {
            let v = Aligned16(i);
            unsafe { bv.push(&v as *const Aligned16 as *const u8) };
        }
        for i in 0..64u128 {
            let ptr = unsafe { bv.get_unchecked(i as usize) };
            assert_eq!(ptr as usize % 16, 0, "alignment broken at index {i}");
            let val = unsafe { *(ptr as *const Aligned16) };
            assert_eq!(val, Aligned16(i));
        }
    }

    #[test]
    fn blobvec_zst() {
        struct Marker;
        let layout = Layout::new::<Marker>();
        assert_eq!(layout.size(), 0);
        let mut bv = BlobVec::new(layout, None);
        for _ in 0..100 {
            unsafe { bv.push(NonNull::dangling().as_ptr()) };
        }
        assert_eq!(bv.len(), 100);
        unsafe { bv.swap_remove_forget(50) };
        assert_eq!(bv.len(), 99);
    }
}
