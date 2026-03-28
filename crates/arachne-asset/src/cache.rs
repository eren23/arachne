/// LRU cache with configurable memory budget and reference-count awareness.
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use crate::handle::{HandleId, RefCount};

struct CacheEntry {
    size: usize,
    ref_count: RefCount,
}

pub struct LruCache {
    /// Maximum bytes allowed in the cache.
    budget: usize,
    /// Current total size of cached assets.
    current_usage: usize,
    /// Cached entries keyed by HandleId.
    entries: HashMap<HandleId, CacheEntry>,
    /// Access order: most recently used at the end.
    access_order: Vec<HandleId>,
}

impl LruCache {
    pub fn new(budget: usize) -> Self {
        LruCache {
            budget,
            current_usage: 0,
            entries: HashMap::new(),
            access_order: Vec::new(),
        }
    }

    /// Current memory usage in bytes.
    pub fn usage(&self) -> usize {
        self.current_usage
    }

    /// Configured budget in bytes.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert an asset into the cache.
    pub fn insert(&mut self, id: HandleId, size: usize, ref_count: RefCount) {
        if self.entries.contains_key(&id) {
            // Already present; just touch it.
            self.touch(id);
            return;
        }
        self.entries.insert(id, CacheEntry { size, ref_count });
        self.access_order.push(id);
        self.current_usage += size;
    }

    /// Mark an asset as recently used.
    pub fn touch(&mut self, id: HandleId) {
        if let Some(pos) = self.access_order.iter().position(|&x| x == id) {
            self.access_order.remove(pos);
            self.access_order.push(id);
        }
    }

    /// Returns true if the asset with the given id is cached.
    pub fn contains(&self, id: HandleId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Remove an asset from the cache.
    pub fn remove(&mut self, id: HandleId) -> bool {
        if let Some(entry) = self.entries.remove(&id) {
            self.current_usage -= entry.size;
            self.access_order.retain(|&x| x != id);
            true
        } else {
            false
        }
    }

    /// Evict least recently used assets until usage is within budget.
    /// Assets with strong handle count > 0 are skipped (kept alive).
    /// Returns the list of evicted HandleIds.
    pub fn evict_if_needed(&mut self) -> Vec<HandleId> {
        let mut evicted = Vec::new();

        while self.current_usage > self.budget {
            // Find the least recently used entry that has no strong handles.
            let mut found = None;
            for &id in &self.access_order {
                if let Some(entry) = self.entries.get(&id) {
                    if entry.ref_count.load(Ordering::Relaxed) == 0 {
                        found = Some(id);
                        break;
                    }
                }
            }

            match found {
                Some(id) => {
                    if let Some(entry) = self.entries.remove(&id) {
                        self.current_usage -= entry.size;
                    }
                    self.access_order.retain(|&x| x != id);
                    evicted.push(id);
                }
                None => {
                    // All remaining assets are in use; cannot evict more.
                    break;
                }
            }
        }

        evicted
    }

    /// Get the access order (LRU first, MRU last). Useful for testing.
    pub fn access_order(&self) -> &[HandleId] {
        &self.access_order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::new_ref_count;
    use std::sync::atomic::Ordering;

    #[test]
    fn basic_insert_and_usage() {
        let mut cache = LruCache::new(1000);
        let rc = new_ref_count();
        cache.insert(HandleId(1), 400, rc.clone());
        assert_eq!(cache.usage(), 400);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(HandleId(1)));
    }

    #[test]
    fn evict_lru_when_over_budget() {
        let mut cache = LruCache::new(1000);

        let rc1 = new_ref_count();
        let rc2 = new_ref_count();
        let rc3 = new_ref_count();

        cache.insert(HandleId(1), 400, rc1.clone());
        cache.insert(HandleId(2), 400, rc2.clone());
        cache.insert(HandleId(3), 400, rc3.clone()); // now at 1200, over budget

        assert_eq!(cache.usage(), 1200);

        let evicted = cache.evict_if_needed();
        // Should evict HandleId(1) first (LRU).
        assert_eq!(evicted, vec![HandleId(1)]);
        assert_eq!(cache.usage(), 800);
        assert!(!cache.contains(HandleId(1)));
        assert!(cache.contains(HandleId(2)));
        assert!(cache.contains(HandleId(3)));
    }

    #[test]
    fn eviction_order_respects_access() {
        let mut cache = LruCache::new(1000);

        let rc1 = new_ref_count();
        let rc2 = new_ref_count();
        let rc3 = new_ref_count();

        cache.insert(HandleId(1), 400, rc1.clone());
        cache.insert(HandleId(2), 400, rc2.clone());
        cache.insert(HandleId(3), 400, rc3.clone());

        // Touch HandleId(1) to make it MRU.
        cache.touch(HandleId(1));

        let evicted = cache.evict_if_needed();
        // HandleId(2) is now LRU, should be evicted first.
        assert_eq!(evicted, vec![HandleId(2)]);
        assert!(cache.contains(HandleId(1)));
        assert!(!cache.contains(HandleId(2)));
        assert!(cache.contains(HandleId(3)));
    }

    #[test]
    fn strong_handles_prevent_eviction() {
        let mut cache = LruCache::new(1000);

        let rc1 = new_ref_count();
        let rc2 = new_ref_count();
        let rc3 = new_ref_count();

        // Simulate a strong handle on HandleId(1).
        rc1.fetch_add(1, Ordering::Relaxed);

        cache.insert(HandleId(1), 400, rc1.clone());
        cache.insert(HandleId(2), 400, rc2.clone());
        cache.insert(HandleId(3), 400, rc3.clone());

        let evicted = cache.evict_if_needed();
        // HandleId(1) has strong refs, so HandleId(2) is evicted instead.
        assert_eq!(evicted, vec![HandleId(2)]);
        assert!(cache.contains(HandleId(1)));
        assert!(!cache.contains(HandleId(2)));
        assert!(cache.contains(HandleId(3)));
    }

    #[test]
    fn cannot_evict_all_in_use() {
        let mut cache = LruCache::new(100);

        let rc1 = new_ref_count();
        let rc2 = new_ref_count();

        // Both have strong handles.
        rc1.fetch_add(1, Ordering::Relaxed);
        rc2.fetch_add(1, Ordering::Relaxed);

        cache.insert(HandleId(1), 200, rc1.clone());
        cache.insert(HandleId(2), 200, rc2.clone());

        let evicted = cache.evict_if_needed();
        // Can't evict anything.
        assert!(evicted.is_empty());
        assert_eq!(cache.usage(), 400);
    }

    #[test]
    fn evict_multiple_to_fit() {
        let mut cache = LruCache::new(500);

        let rcs: Vec<RefCount> = (0..5).map(|_| new_ref_count()).collect();
        for i in 0..5 {
            cache.insert(HandleId(i as u64), 200, rcs[i].clone());
        }
        // 5 * 200 = 1000, budget = 500. Need to evict at least 3.
        let evicted = cache.evict_if_needed();
        assert_eq!(evicted.len(), 3);
        assert_eq!(evicted, vec![HandleId(0), HandleId(1), HandleId(2)]);
        assert_eq!(cache.usage(), 400);
    }

    #[test]
    fn remove_explicit() {
        let mut cache = LruCache::new(1000);
        let rc = new_ref_count();
        cache.insert(HandleId(1), 500, rc.clone());
        assert_eq!(cache.usage(), 500);

        assert!(cache.remove(HandleId(1)));
        assert_eq!(cache.usage(), 0);
        assert!(!cache.contains(HandleId(1)));

        // Removing non-existent returns false.
        assert!(!cache.remove(HandleId(99)));
    }

    #[test]
    fn duplicate_insert_is_touch() {
        let mut cache = LruCache::new(2000);
        let rc1 = new_ref_count();
        let rc2 = new_ref_count();

        cache.insert(HandleId(1), 100, rc1.clone());
        cache.insert(HandleId(2), 100, rc2.clone());

        // Re-insert HandleId(1) — should just touch, not add size.
        cache.insert(HandleId(1), 100, rc1.clone());
        assert_eq!(cache.usage(), 200);
        assert_eq!(cache.len(), 2);
        // HandleId(1) should now be MRU.
        assert_eq!(*cache.access_order().last().unwrap(), HandleId(1));
    }
}
