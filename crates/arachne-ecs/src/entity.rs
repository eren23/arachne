/// Lightweight entity handle: index + generation for use-after-free detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Entity {
    /// Create an entity handle (mainly for tests / internal use).
    #[inline]
    pub fn from_raw(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    #[inline]
    pub fn index(self) -> u32 {
        self.index
    }

    #[inline]
    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// Manages entity allocation with generational indices and a free list.
pub struct EntityAllocator {
    /// One generation counter per ever-allocated index.
    generations: Vec<u32>,
    /// Recycled indices available for reuse.
    free_list: Vec<u32>,
    /// Total number of currently alive entities.
    alive_count: u32,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free_list: Vec::new(),
            alive_count: 0,
        }
    }

    /// Allocate a fresh entity. Reuses recycled indices when available.
    pub fn allocate(&mut self) -> Entity {
        self.alive_count += 1;
        if let Some(index) = self.free_list.pop() {
            // Reuse a recycled slot – generation was already bumped on deallocation.
            Entity {
                index,
                generation: self.generations[index as usize],
            }
        } else {
            // Brand-new index.
            let index = self.generations.len() as u32;
            self.generations.push(0);
            Entity {
                index,
                generation: 0,
            }
        }
    }

    /// Deallocate an entity. Returns `false` if the entity was already dead.
    pub fn deallocate(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        // Bump generation so stale handles are detected.
        self.generations[entity.index as usize] += 1;
        self.free_list.push(entity.index);
        self.alive_count -= 1;
        true
    }

    /// Check whether an entity handle still refers to a living entity.
    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        (entity.index as usize) < self.generations.len()
            && self.generations[entity.index as usize] == entity.generation
    }

    #[inline]
    pub fn alive_count(&self) -> u32 {
        self.alive_count
    }

    /// Total slots ever allocated (including recycled).
    #[inline]
    pub fn total_slots(&self) -> u32 {
        self.generations.len() as u32
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn allocate_and_check_alive() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        assert!(alloc.is_alive(e));
        assert_eq!(e.index(), 0);
        assert_eq!(e.generation(), 0);
    }

    #[test]
    fn deallocate_makes_dead() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        assert!(alloc.deallocate(e));
        assert!(!alloc.is_alive(e));
    }

    #[test]
    fn generation_increments_on_dealloc() {
        let mut alloc = EntityAllocator::new();
        let e1 = alloc.allocate();
        alloc.deallocate(e1);
        let e2 = alloc.allocate();
        // Same index, different generation.
        assert_eq!(e1.index(), e2.index());
        assert_ne!(e1.generation(), e2.generation());
        assert_eq!(e2.generation(), 1);
        // Old handle is stale.
        assert!(!alloc.is_alive(e1));
        assert!(alloc.is_alive(e2));
    }

    #[test]
    fn double_deallocate_returns_false() {
        let mut alloc = EntityAllocator::new();
        let e = alloc.allocate();
        assert!(alloc.deallocate(e));
        assert!(!alloc.deallocate(e));
    }

    #[test]
    fn million_spawn_despawn_no_collisions() {
        let mut alloc = EntityAllocator::new();
        let mut seen = HashSet::new();

        // Allocate 1M entities.
        let mut entities: Vec<Entity> = (0..1_000_000).map(|_| alloc.allocate()).collect();
        for &e in &entities {
            assert!(
                seen.insert((e.index(), e.generation())),
                "collision on first pass"
            );
        }

        // Deallocate all.
        for &e in &entities {
            assert!(alloc.deallocate(e));
        }
        assert_eq!(alloc.alive_count(), 0);

        // Reallocate 1M – all must have new generations.
        entities.clear();
        seen.clear();
        for _ in 0..1_000_000 {
            let e = alloc.allocate();
            assert!(
                seen.insert((e.index(), e.generation())),
                "collision on second pass"
            );
            entities.push(e);
        }
        assert_eq!(alloc.alive_count(), 1_000_000);
    }
}
