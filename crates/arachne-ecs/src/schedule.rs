use crate::system::{IntoSystem, System};
use crate::world::World;
use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// Stage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Startup,
    PreUpdate,
    Update,
    PostUpdate,
    Render,
}

impl Stage {
    /// Ordered list of runtime stages (Startup excluded – it runs only once).
    pub const RUNTIME_ORDER: &'static [Stage] = &[
        Stage::PreUpdate,
        Stage::Update,
        Stage::PostUpdate,
        Stage::Render,
    ];
}

// ---------------------------------------------------------------------------
// SystemEntry – internal bookkeeping per system
// ---------------------------------------------------------------------------

struct SystemEntry {
    system: Box<dyn System>,
    before: Vec<String>,
    after: Vec<String>,
}

// ---------------------------------------------------------------------------
// Schedule
// ---------------------------------------------------------------------------

pub struct Schedule {
    stages: HashMap<Stage, Vec<SystemEntry>>,
    /// Cached execution order per stage (invalidated on add_system).
    order_cache: HashMap<Stage, Vec<usize>>,
    startup_done: bool,
}

impl Schedule {
    pub fn new() -> Self {
        let mut stages = HashMap::new();
        stages.insert(Stage::Startup, Vec::new());
        stages.insert(Stage::PreUpdate, Vec::new());
        stages.insert(Stage::Update, Vec::new());
        stages.insert(Stage::PostUpdate, Vec::new());
        stages.insert(Stage::Render, Vec::new());
        Self {
            stages,
            order_cache: HashMap::new(),
            startup_done: false,
        }
    }

    /// Add a system to a stage. Returns a `SystemBuilder` for declaring
    /// ordering constraints (`.before()` / `.after()`).
    pub fn add_system<M>(
        &mut self,
        stage: Stage,
        system: impl IntoSystem<M>,
    ) -> SystemBuilder<'_> {
        let sys = system.into_system();
        let entry = SystemEntry {
            system: Box::new(sys),
            before: Vec::new(),
            after: Vec::new(),
        };
        let systems = self.stages.get_mut(&stage).unwrap();
        let idx = systems.len();
        systems.push(entry);
        // Invalidate cache for this stage.
        self.order_cache.remove(&stage);
        SystemBuilder {
            schedule: self,
            stage,
            idx,
        }
    }

    /// Run one frame: startup (first time only), then runtime stages in order.
    /// Increments world tick, swaps events, and calls `apply_deferred`
    /// between stages.
    pub fn run(&mut self, world: &mut World) {
        world.tick += 1;

        // Startup systems run exactly once.
        if !self.startup_done {
            self.startup_done = true;
            self.run_stage(Stage::Startup, world);
            apply_deferred(world);
        }

        // Swap event buffers at start of frame so systems read last frame's events.
        world.events.swap_all();

        for &stage in Stage::RUNTIME_ORDER {
            self.run_stage(stage, world);
            apply_deferred(world);
        }
    }

    fn run_stage(&mut self, stage: Stage, world: &mut World) {
        // Compute and cache order if not already cached.
        if !self.order_cache.contains_key(&stage) {
            let systems = self.stages.get(&stage).unwrap();
            let order = topological_sort(systems);
            self.order_cache.insert(stage, order);
        }
        let order = self.order_cache.get(&stage).unwrap().clone();
        let systems = self.stages.get_mut(&stage).unwrap();
        for idx in order {
            systems[idx].system.run(world);
        }
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SystemBuilder – fluent API for ordering
// ---------------------------------------------------------------------------

pub struct SystemBuilder<'a> {
    schedule: &'a mut Schedule,
    stage: Stage,
    idx: usize,
}

impl<'a> SystemBuilder<'a> {
    /// Declare that this system should run **before** a named system.
    pub fn before(self, name: &str) -> Self {
        self.schedule.stages.get_mut(&self.stage).unwrap()[self.idx]
            .before
            .push(name.to_string());
        // Invalidate cache.
        self.schedule.order_cache.remove(&self.stage);
        self
    }

    /// Declare that this system should run **after** a named system.
    pub fn after(self, name: &str) -> Self {
        self.schedule.stages.get_mut(&self.stage).unwrap()[self.idx]
            .after
            .push(name.to_string());
        // Invalidate cache.
        self.schedule.order_cache.remove(&self.stage);
        self
    }
}

// ---------------------------------------------------------------------------
// apply_deferred – flush pending commands
// ---------------------------------------------------------------------------

/// Apply all deferred commands (spawns, despawns, inserts, etc.) that systems
/// have queued via `Commands`.
pub fn apply_deferred(world: &mut World) {
    // Take the queue to avoid borrow conflict.
    let mut queue = std::mem::take(&mut world.command_queue);
    queue.apply(world);
    // Restore the (now-empty) queue.
    world.command_queue = queue;
}

// ---------------------------------------------------------------------------
// Topological sort with cycle detection
// ---------------------------------------------------------------------------

fn topological_sort(systems: &[SystemEntry]) -> Vec<usize> {
    let n = systems.len();
    if n == 0 {
        return Vec::new();
    }

    // Build name → index map.
    let name_to_idx: HashMap<&str, usize> = systems
        .iter()
        .enumerate()
        .map(|(i, s)| (s.system.name(), i))
        .collect();

    // Build adjacency list: edges[a] contains b means a must run before b.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree: Vec<usize> = vec![0; n];

    for (i, entry) in systems.iter().enumerate() {
        // "i.before(name)" → edge from i → name_to_idx[name]
        for before_name in &entry.before {
            if let Some(&j) = name_to_idx.get(before_name.as_str()) {
                adj[i].push(j);
                in_degree[j] += 1;
            }
        }
        // "i.after(name)" → edge from name_to_idx[name] → i
        for after_name in &entry.after {
            if let Some(&j) = name_to_idx.get(after_name.as_str()) {
                adj[j].push(i);
                in_degree[i] += 1;
            }
        }
    }

    // Kahn's algorithm.
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .enumerate()
        .filter(|(_, &d)| d == 0)
        .map(|(i, _)| i)
        .collect();

    let mut order = Vec::with_capacity(n);
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for &neighbor in &adj[node] {
            in_degree[neighbor] -= 1;
            if in_degree[neighbor] == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if order.len() != n {
        // Find systems involved in cycle for error message.
        let in_cycle: Vec<&str> = (0..n)
            .filter(|i| in_degree[*i] > 0)
            .map(|i| systems[i].system.name())
            .collect();
        panic!(
            "cycle detected in system ordering: [{}]",
            in_cycle.join(", ")
        );
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topological_sort_empty() {
        let systems: Vec<SystemEntry> = Vec::new();
        let order = topological_sort(&systems);
        assert!(order.is_empty());
    }
}
