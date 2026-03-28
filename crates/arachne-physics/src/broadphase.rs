use std::collections::HashMap;

use arachne_math::{Rect, Vec2};

use crate::rigid_body::BodyHandle;

/// Spatial hash grid for broad-phase collision detection.
///
/// Maps world-space AABBs into grid cells and efficiently returns
/// potentially-overlapping pairs. No duplicate pairs, no self-pairs.
pub struct SpatialHashGrid {
    pub cell_size: f32,
    inv_cell_size: f32,
    cells: HashMap<(i32, i32), Vec<BodyHandle>>,
}

impl SpatialHashGrid {
    /// Creates a new spatial hash grid with the given cell size.
    pub fn new(cell_size: f32) -> Self {
        assert!(cell_size > 0.0);
        Self {
            cell_size,
            inv_cell_size: 1.0 / cell_size,
            cells: HashMap::new(),
        }
    }

    /// Clears all entries from the grid.
    #[inline]
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Converts a world-space coordinate to grid cell coordinate.
    #[inline]
    fn cell_coord(&self, val: f32) -> i32 {
        (val * self.inv_cell_size).floor() as i32
    }

    /// Inserts a body's AABB into the grid.
    pub fn insert(&mut self, handle: BodyHandle, aabb: Rect) {
        let min_x = self.cell_coord(aabb.min.x);
        let min_y = self.cell_coord(aabb.min.y);
        let max_x = self.cell_coord(aabb.max.x);
        let max_y = self.cell_coord(aabb.max.y);

        for cx in min_x..=max_x {
            for cy in min_y..=max_y {
                self.cells.entry((cx, cy)).or_default().push(handle);
            }
        }
    }

    /// Returns all potentially-colliding pairs. No duplicates, no self-pairs.
    pub fn query_pairs(&self) -> Vec<(BodyHandle, BodyHandle)> {
        // Use a set to deduplicate pairs.
        let mut pair_set = HashMap::<(u32, u32), ()>::new();

        for cell in self.cells.values() {
            let n = cell.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    let a = cell[i].0;
                    let b = cell[j].0;
                    let key = if a < b { (a, b) } else { (b, a) };
                    pair_set.entry(key).or_default();
                }
            }
        }

        pair_set
            .into_keys()
            .map(|(a, b)| (BodyHandle(a), BodyHandle(b)))
            .collect()
    }

    /// Returns all body handles in cells that overlap the given AABB.
    pub fn query_aabb(&self, aabb: Rect) -> Vec<BodyHandle> {
        let min_x = self.cell_coord(aabb.min.x);
        let min_y = self.cell_coord(aabb.min.y);
        let max_x = self.cell_coord(aabb.max.x);
        let max_y = self.cell_coord(aabb.max.y);

        let mut seen = HashMap::<u32, ()>::new();
        let mut result = Vec::new();

        for cx in min_x..=max_x {
            for cy in min_y..=max_y {
                if let Some(cell) = self.cells.get(&(cx, cy)) {
                    for &handle in cell {
                        if seen.insert(handle.0, ()).is_none() {
                            result.push(handle);
                        }
                    }
                }
            }
        }

        result
    }

    /// Returns all body handles in cells that contain the given point.
    pub fn query_point(&self, point: Vec2) -> Vec<BodyHandle> {
        let cx = self.cell_coord(point.x);
        let cy = self.cell_coord(point.y);
        self.cells.get(&(cx, cy)).cloned().unwrap_or_default()
    }

    /// Returns the cells as rectangles for debug drawing.
    pub fn debug_cells(&self) -> Vec<Rect> {
        self.cells
            .keys()
            .map(|&(cx, cy)| {
                let min = Vec2::new(cx as f32 * self.cell_size, cy as f32 * self.cell_size);
                let max = min + Vec2::splat(self.cell_size);
                Rect::new(min, max)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query_pair() {
        let mut grid = SpatialHashGrid::new(10.0);
        // Two overlapping AABBs in the same cell
        grid.insert(
            BodyHandle(0),
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0)),
        );
        grid.insert(
            BodyHandle(1),
            Rect::new(Vec2::new(3.0, 3.0), Vec2::new(8.0, 8.0)),
        );
        let pairs = grid.query_pairs();
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn no_self_pairs() {
        let mut grid = SpatialHashGrid::new(10.0);
        grid.insert(
            BodyHandle(0),
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0)),
        );
        let pairs = grid.query_pairs();
        assert!(pairs.is_empty());
    }

    #[test]
    fn no_duplicate_pairs() {
        let mut grid = SpatialHashGrid::new(5.0);
        // Bodies spanning multiple cells
        grid.insert(
            BodyHandle(0),
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
        );
        grid.insert(
            BodyHandle(1),
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
        );
        let pairs = grid.query_pairs();
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn separated_bodies_no_pairs() {
        let mut grid = SpatialHashGrid::new(10.0);
        grid.insert(
            BodyHandle(0),
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0)),
        );
        grid.insert(
            BodyHandle(1),
            Rect::new(Vec2::new(100.0, 100.0), Vec2::new(105.0, 105.0)),
        );
        let pairs = grid.query_pairs();
        assert!(pairs.is_empty());
    }

    #[test]
    fn zero_false_negatives_vs_brute_force() {
        use arachne_math::Vec2;

        let cell_size = 10.0;
        let mut grid = SpatialHashGrid::new(cell_size);

        // Use a simple LCG for deterministic random positions
        let mut seed: u32 = 12345;
        let mut next_rand = || -> f32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            ((seed >> 16) as f32 / 65535.0) * 100.0 - 50.0
        };

        let n = 100;
        let mut aabbs = Vec::new();
        for i in 0..n {
            let x = next_rand();
            let y = next_rand();
            let aabb = Rect::new(Vec2::new(x, y), Vec2::new(x + 3.0, y + 3.0));
            grid.insert(BodyHandle(i), aabb);
            aabbs.push(aabb);
        }

        let pairs = grid.query_pairs();
        let pair_set: std::collections::HashSet<(u32, u32)> = pairs
            .iter()
            .map(|&(a, b)| if a.0 < b.0 { (a.0, b.0) } else { (b.0, a.0) })
            .collect();

        // Brute force
        for i in 0..n {
            for j in (i + 1)..n {
                if aabbs[i as usize].intersects(aabbs[j as usize]) {
                    let key = (i, j);
                    assert!(
                        pair_set.contains(&key),
                        "False negative: ({}, {})",
                        i,
                        j
                    );
                }
            }
        }
    }

    #[test]
    fn query_aabb_returns_contained_bodies() {
        let mut grid = SpatialHashGrid::new(10.0);
        grid.insert(
            BodyHandle(0),
            Rect::new(Vec2::new(5.0, 5.0), Vec2::new(8.0, 8.0)),
        );
        grid.insert(
            BodyHandle(1),
            Rect::new(Vec2::new(50.0, 50.0), Vec2::new(55.0, 55.0)),
        );
        let results = grid.query_aabb(Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)));
        assert!(results.iter().any(|h| h.0 == 0));
        assert!(!results.iter().any(|h| h.0 == 1));
    }

    #[test]
    fn clear_empties_grid() {
        let mut grid = SpatialHashGrid::new(10.0);
        grid.insert(
            BodyHandle(0),
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0)),
        );
        grid.clear();
        let pairs = grid.query_pairs();
        assert!(pairs.is_empty());
    }

    #[test]
    fn bench_broadphase_pair_checks() {
        use std::hint::black_box;
        use std::time::Instant;

        let cell_size = 10.0;
        let n = 1000u32;
        let mut seed: u32 = 42;
        let mut next_rand = || -> f32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            ((seed >> 16) as f32 / 65535.0) * 200.0 - 100.0
        };

        let mut aabbs = Vec::new();
        for _ in 0..n {
            let x = next_rand();
            let y = next_rand();
            aabbs.push(Rect::new(Vec2::new(x, y), Vec2::new(x + 2.0, y + 2.0)));
        }

        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let mut grid = SpatialHashGrid::new(cell_size);
            for (i, aabb) in aabbs.iter().enumerate() {
                grid.insert(BodyHandle(i as u32), *aabb);
            }
            let pairs = black_box(grid.query_pairs());
            black_box(pairs.len());
        }
        let elapsed = start.elapsed();

        // Each iteration checks all potential pairs in the grid
        // With 1000 bodies, the brute-force pair count would be ~500K
        // We want broadphase to handle >= 1M pair checks/sec
        let total_pair_checks = n as u64 * (n as u64 - 1) / 2 * iterations as u64;
        let checks_per_sec = total_pair_checks as f64 / elapsed.as_secs_f64();
        eprintln!(
            "Broadphase: {:.1}M pair checks/sec ({} iterations in {:.3}ms)",
            checks_per_sec / 1_000_000.0,
            iterations,
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            checks_per_sec >= 1_000_000.0,
            "Broadphase throughput {:.1}M < 1M pair checks/sec",
            checks_per_sec / 1_000_000.0
        );
    }
}
