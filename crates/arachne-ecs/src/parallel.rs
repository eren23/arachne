// ---------------------------------------------------------------------------
// parallel.rs – Optional system parallelism (native only)
// ---------------------------------------------------------------------------
//
// Provides:
// - ThreadPool: a scoped thread pool
// - ParallelIterator: chunk-based parallel iteration over slices
// - SystemAccessInfo: describes what resources/components a system accesses
// - ParallelScheduler: determines which systems can run concurrently

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// System access tracking (works on all targets)
// ---------------------------------------------------------------------------

/// Describes the resource and component access pattern of a system.
#[derive(Clone, Debug, Default)]
pub struct SystemAccessInfo {
    /// Component types read immutably.
    pub reads: HashSet<u64>,
    /// Component types written mutably.
    pub writes: HashSet<u64>,
    /// Resource types read immutably.
    pub resource_reads: HashSet<u64>,
    /// Resource types written mutably.
    pub resource_writes: HashSet<u64>,
}

impl SystemAccessInfo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_component_read(&mut self, type_hash: u64) {
        self.reads.insert(type_hash);
    }

    pub fn add_component_write(&mut self, type_hash: u64) {
        self.writes.insert(type_hash);
    }

    pub fn add_resource_read(&mut self, type_hash: u64) {
        self.resource_reads.insert(type_hash);
    }

    pub fn add_resource_write(&mut self, type_hash: u64) {
        self.resource_writes.insert(type_hash);
    }

    /// Returns `true` if this system's access conflicts with `other`.
    pub fn conflicts_with(&self, other: &SystemAccessInfo) -> bool {
        if !self.writes.is_disjoint(&other.writes) {
            return true;
        }
        if !self.reads.is_disjoint(&other.writes) {
            return true;
        }
        if !self.writes.is_disjoint(&other.reads) {
            return true;
        }
        if !self.resource_writes.is_disjoint(&other.resource_writes) {
            return true;
        }
        if !self.resource_reads.is_disjoint(&other.resource_writes) {
            return true;
        }
        if !self.resource_writes.is_disjoint(&other.resource_reads) {
            return true;
        }
        false
    }

    /// Returns `true` if this access is read-only.
    pub fn is_read_only(&self) -> bool {
        self.writes.is_empty() && self.resource_writes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Parallel system batching
// ---------------------------------------------------------------------------

/// Compute batches of systems that can safely execute in parallel.
pub fn compute_parallel_batches(accesses: &[SystemAccessInfo]) -> Vec<Vec<usize>> {
    let n = accesses.len();
    if n == 0 {
        return Vec::new();
    }

    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![false; n];

    for i in 0..n {
        if assigned[i] {
            continue;
        }

        let mut batch = vec![i];
        assigned[i] = true;

        for j in (i + 1)..n {
            if assigned[j] {
                continue;
            }
            let conflicts = batch
                .iter()
                .any(|&k| accesses[j].conflicts_with(&accesses[k]));
            if !conflicts {
                batch.push(j);
                assigned[j] = true;
            }
        }

        batches.push(batch);
    }

    batches
}

// ---------------------------------------------------------------------------
// ThreadPool – uses std::thread::scope for safe scoped parallelism
// ---------------------------------------------------------------------------

/// A simple thread pool. On native targets, uses `std::thread::scope` for
/// safe scoped parallelism. On WASM, executes everything sequentially.
pub struct ThreadPool {
    thread_count: usize,
}

impl ThreadPool {
    pub fn new(num_threads: usize) -> Self {
        Self {
            thread_count: num_threads.max(1),
        }
    }

    /// Number of worker threads.
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }

    /// Run a scoped parallel section. All spawned work completes before
    /// this function returns.
    pub fn scope<'env, F, R>(&self, f: F) -> R
    where
        F: for<'scope> FnOnce(&'scope std::thread::Scope<'scope, 'env>) -> R,
    {
        std::thread::scope(f)
    }
}

// ---------------------------------------------------------------------------
// Parallel iterator utilities
// ---------------------------------------------------------------------------

/// Splits a slice into chunks and processes each chunk with a closure,
/// potentially in parallel.
pub fn par_for_each<T: Send + Sync>(
    pool: &ThreadPool,
    data: &[T],
    chunk_size: usize,
    f: impl Fn(&T) + Send + Sync,
) {
    let chunk_size = chunk_size.max(1);

    if data.len() <= chunk_size || pool.thread_count() <= 1 {
        for item in data {
            f(item);
        }
        return;
    }

    pool.scope(|scope| {
        for chunk in data.chunks(chunk_size) {
            let f = &f;
            scope.spawn(move || {
                for item in chunk {
                    f(item);
                }
            });
        }
    });
}

/// Mutable version: splits a mutable slice into chunks for parallel mutation.
pub fn par_for_each_mut<T: Send + Sync>(
    pool: &ThreadPool,
    data: &mut [T],
    chunk_size: usize,
    f: impl Fn(&mut T) + Send + Sync,
) {
    let chunk_size = chunk_size.max(1);

    if data.len() <= chunk_size || pool.thread_count() <= 1 {
        for item in data.iter_mut() {
            f(item);
        }
        return;
    }

    pool.scope(|scope| {
        for chunk in data.chunks_mut(chunk_size) {
            let f = &f;
            scope.spawn(move || {
                for item in chunk.iter_mut() {
                    f(item);
                }
            });
        }
    });
}

/// Map in parallel, collecting results into a Vec.
pub fn par_map<T: Send + Sync, R: Send>(
    pool: &ThreadPool,
    data: &[T],
    chunk_size: usize,
    f: impl Fn(&T) -> R + Send + Sync,
) -> Vec<R> {
    let chunk_size = chunk_size.max(1);

    if data.len() <= chunk_size || pool.thread_count() <= 1 {
        return data.iter().map(|x| f(x)).collect();
    }

    let chunk_results: std::sync::Mutex<Vec<(usize, Vec<R>)>> =
        std::sync::Mutex::new(Vec::new());

    pool.scope(|scope| {
        for (chunk_idx, chunk) in data.chunks(chunk_size).enumerate() {
            let f = &f;
            let results = &chunk_results;
            scope.spawn(move || {
                let partial: Vec<R> = chunk.iter().map(|x| f(x)).collect();
                results.lock().unwrap().push((chunk_idx, partial));
            });
        }
    });

    let mut indexed = chunk_results.into_inner().unwrap();
    indexed.sort_by_key(|(idx, _)| *idx);

    let mut result = Vec::with_capacity(data.len());
    for (_, chunk) in indexed {
        result.extend(chunk);
    }
    result
}

// ---------------------------------------------------------------------------
// Scope (re-export for API compatibility)
// ---------------------------------------------------------------------------

/// Type alias for std::thread::Scope for use in the public API.
pub type Scope<'scope, 'env> = std::thread::Scope<'scope, 'env>;

// ---------------------------------------------------------------------------
// ParallelScheduler
// ---------------------------------------------------------------------------

/// Schedules systems within a stage for parallel execution.
pub struct ParallelScheduler {
    batches: Vec<Vec<usize>>,
}

impl ParallelScheduler {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
        }
    }

    pub fn rebuild(&mut self, accesses: &[SystemAccessInfo]) {
        self.batches = compute_parallel_batches(accesses);
    }

    pub fn batches(&self) -> &[Vec<usize>] {
        &self.batches
    }

    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn is_fully_parallel(&self) -> bool {
        self.batches.len() <= 1
    }
}

impl Default for ParallelScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_conflict_disjoint_reads() {
        let mut a = SystemAccessInfo::new();
        a.add_component_read(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_read(2);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn no_conflict_same_reads() {
        let mut a = SystemAccessInfo::new();
        a.add_component_read(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_read(1);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn conflict_read_write_same_component() {
        let mut a = SystemAccessInfo::new();
        a.add_component_read(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_write(1);
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn conflict_write_write_same_component() {
        let mut a = SystemAccessInfo::new();
        a.add_component_write(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_write(1);
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn no_conflict_write_different_components() {
        let mut a = SystemAccessInfo::new();
        a.add_component_write(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_write(2);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn conflict_resource_read_write() {
        let mut a = SystemAccessInfo::new();
        a.add_resource_read(10);
        let mut b = SystemAccessInfo::new();
        b.add_resource_write(10);
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn no_conflict_resource_reads() {
        let mut a = SystemAccessInfo::new();
        a.add_resource_read(10);
        let mut b = SystemAccessInfo::new();
        b.add_resource_read(10);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn is_read_only() {
        let mut a = SystemAccessInfo::new();
        a.add_component_read(1);
        a.add_resource_read(2);
        assert!(a.is_read_only());

        a.add_component_write(3);
        assert!(!a.is_read_only());
    }

    #[test]
    fn batch_empty_systems() {
        let batches = compute_parallel_batches(&[]);
        assert!(batches.is_empty());
    }

    #[test]
    fn batch_single_system() {
        let access = SystemAccessInfo::new();
        let batches = compute_parallel_batches(&[access]);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![0]);
    }

    #[test]
    fn batch_two_non_conflicting_systems() {
        let mut a = SystemAccessInfo::new();
        a.add_component_read(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_read(2);

        let batches = compute_parallel_batches(&[a, b]);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 2);
    }

    #[test]
    fn batch_two_conflicting_systems() {
        let mut a = SystemAccessInfo::new();
        a.add_component_write(1);
        let mut b = SystemAccessInfo::new();
        b.add_component_write(1);

        let batches = compute_parallel_batches(&[a, b]);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn batch_mixed_conflict_no_conflict() {
        let mut s0 = SystemAccessInfo::new();
        s0.add_component_read(1);
        let mut s1 = SystemAccessInfo::new();
        s1.add_component_write(1);
        let mut s2 = SystemAccessInfo::new();
        s2.add_component_read(2);
        let mut s3 = SystemAccessInfo::new();
        s3.add_component_read(1);

        let batches = compute_parallel_batches(&[s0, s1, s2, s3]);

        let mut found_s1_batch = false;
        for batch in &batches {
            if batch.contains(&1) {
                found_s1_batch = true;
                assert!(!batch.contains(&0));
                assert!(!batch.contains(&3));
            }
        }
        assert!(found_s1_batch);

        let all: Vec<usize> = batches.iter().flat_map(|b| b.iter().copied()).collect();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn scheduler_rebuild() {
        let mut scheduler = ParallelScheduler::new();
        let mut s0 = SystemAccessInfo::new();
        s0.add_component_read(1);
        let mut s1 = SystemAccessInfo::new();
        s1.add_component_read(2);

        scheduler.rebuild(&[s0, s1]);
        assert!(scheduler.is_fully_parallel());
        assert_eq!(scheduler.batch_count(), 1);
    }

    #[test]
    fn thread_pool_creation() {
        let pool = ThreadPool::new(4);
        assert_eq!(pool.thread_count(), 4);
    }

    #[test]
    fn thread_pool_scope_executes_all() {
        let pool = ThreadPool::new(2);
        let counter = std::sync::atomic::AtomicU32::new(0);

        pool.scope(|scope| {
            for _ in 0..10 {
                let c = &counter;
                scope.spawn(move || {
                    c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                });
            }
        });

        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 10);
    }

    #[test]
    fn par_for_each_sums_correctly() {
        let pool = ThreadPool::new(2);
        let data: Vec<u32> = (0..100).collect();
        let sum = std::sync::atomic::AtomicU64::new(0);

        par_for_each(&pool, &data, 16, |&x| {
            sum.fetch_add(x as u64, std::sync::atomic::Ordering::Relaxed);
        });

        let expected: u64 = (0..100u64).sum();
        assert_eq!(sum.load(std::sync::atomic::Ordering::Relaxed), expected);
    }

    #[test]
    fn par_for_each_mut_doubles_values() {
        let pool = ThreadPool::new(2);
        let mut data: Vec<u32> = (0..50).collect();

        par_for_each_mut(&pool, &mut data, 8, |x| {
            *x *= 2;
        });

        for (i, &val) in data.iter().enumerate() {
            assert_eq!(val, (i as u32) * 2);
        }
    }

    #[test]
    fn par_map_squares() {
        let pool = ThreadPool::new(2);
        let data: Vec<u32> = (0..20).collect();

        let result = par_map(&pool, &data, 4, |&x| x * x);

        assert_eq!(result.len(), 20);
        for (i, &val) in result.iter().enumerate() {
            assert_eq!(val, (i as u32) * (i as u32));
        }
    }

    #[test]
    fn par_for_each_small_slice_sequential() {
        let pool = ThreadPool::new(2);
        let data = vec![1u32, 2, 3];
        let sum = std::sync::atomic::AtomicU64::new(0);

        par_for_each(&pool, &data, 100, |&x| {
            sum.fetch_add(x as u64, std::sync::atomic::Ordering::Relaxed);
        });

        assert_eq!(sum.load(std::sync::atomic::Ordering::Relaxed), 6);
    }

    #[test]
    fn par_for_each_empty_slice() {
        let pool = ThreadPool::new(2);
        let data: Vec<u32> = vec![];

        par_for_each(&pool, &data, 16, |_| {
            panic!("should not be called");
        });
    }

    #[test]
    fn bench_parallel_iteration_1m_items() {
        let pool = ThreadPool::new(4);
        let data: Vec<f32> = (0..1_000_000).map(|i| i as f32).collect();
        let sum = std::sync::atomic::AtomicU64::new(0);

        let start = std::time::Instant::now();
        par_for_each(&pool, &data, 64_000, |&x| {
            let bits = (x as u64) & 0xFF;
            sum.fetch_add(bits, std::sync::atomic::Ordering::Relaxed);
        });
        let elapsed = start.elapsed();

        eprintln!(
            "par_for_each 1M items: {:.2}ms",
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(sum.load(std::sync::atomic::Ordering::Relaxed) > 0);
    }
}
