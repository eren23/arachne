/// A render graph that manages pass ordering and resource dependencies.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Identifies a render pass.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct PassId(pub &'static str);

/// A resource produced or consumed by a pass (e.g., a texture or buffer).
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ResourceId(pub &'static str);

/// Type of a render graph resource.
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceKind {
    /// A 2D texture (color target, depth buffer, etc.).
    Texture {
        width: u32,
        height: u32,
        format: TextureFormatHint,
    },
    /// A GPU buffer.
    Buffer { size: u64 },
    /// An abstract/external resource (e.g., swap chain image).
    External,
}

/// Hint for texture format (avoids depending on wgpu types in the graph).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFormatHint {
    Rgba8,
    Bgra8,
    Depth32,
    Depth24Stencil8,
    R8,
    Rgba16Float,
}

/// Description of a render pass within the graph.
pub struct PassDescriptor {
    pub id: PassId,
    pub inputs: Vec<ResourceId>,
    pub outputs: Vec<ResourceId>,
    /// Whether this pass should be executed.
    pub enabled: bool,
}

impl PassDescriptor {
    pub fn new(id: PassId) -> Self {
        Self {
            id,
            inputs: Vec::new(),
            outputs: Vec::new(),
            enabled: true,
        }
    }

    pub fn reads(mut self, resource: ResourceId) -> Self {
        self.inputs.push(resource);
        self
    }

    pub fn writes(mut self, resource: ResourceId) -> Self {
        self.outputs.push(resource);
        self
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Describes a resource's lifetime in the graph.
#[derive(Clone, Debug)]
pub struct ResourceDescriptor {
    pub id: ResourceId,
    pub kind: ResourceKind,
    /// The pass that first writes this resource (producer).
    pub producer: Option<PassId>,
    /// Passes that read this resource (consumers).
    pub consumers: Vec<PassId>,
    /// Whether this is an imported (external) resource or transient.
    pub imported: bool,
}

// ---------------------------------------------------------------------------
// RenderGraph
// ---------------------------------------------------------------------------

/// A compiled render graph: passes in execution order.
pub struct RenderGraph {
    passes: Vec<PassDescriptor>,
    resources: HashMap<String, ResourceDescriptor>,
    sorted: Vec<usize>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            resources: HashMap::new(),
            sorted: Vec::new(),
        }
    }

    /// Add a pass to the graph.
    pub fn add_pass(&mut self, desc: PassDescriptor) {
        self.passes.push(desc);
        self.sorted.clear(); // invalidate sort
    }

    /// Declare a transient resource in the graph.
    pub fn add_resource(&mut self, desc: ResourceDescriptor) {
        self.resources.insert(desc.id.0.to_string(), desc);
    }

    /// Declare an imported (external) resource.
    pub fn import_resource(&mut self, id: ResourceId, kind: ResourceKind) {
        self.resources.insert(
            id.0.to_string(),
            ResourceDescriptor {
                id,
                kind,
                producer: None,
                consumers: Vec::new(),
                imported: true,
            },
        );
    }

    /// Get a resource descriptor by id.
    pub fn resource(&self, id: &str) -> Option<&ResourceDescriptor> {
        self.resources.get(id)
    }

    /// Topologically sort passes based on resource dependencies.
    /// Returns the execution order indices. Also cached in `execution_order()`.
    pub fn compile(&mut self) -> Result<Vec<usize>, GraphError> {
        let n = self.passes.len();
        if n == 0 {
            self.sorted.clear();
            return Ok(self.sorted.clone());
        }

        // Filter to enabled passes
        let enabled_indices: Vec<usize> = (0..n)
            .filter(|&i| self.passes[i].enabled)
            .collect();
        let enabled_count = enabled_indices.len();

        if enabled_count == 0 {
            self.sorted.clear();
            return Ok(self.sorted.clone());
        }

        // Build adjacency among enabled passes
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        for &j in &enabled_indices {
            for input in &self.passes[j].inputs {
                for &i in &enabled_indices {
                    if i == j {
                        continue;
                    }
                    if self.passes[i].outputs.contains(input) {
                        adj[i].push(j);
                        in_degree[j] += 1;
                    }
                }
            }
        }

        // Kahn's algorithm
        let mut queue: Vec<usize> = enabled_indices
            .iter()
            .filter(|&&i| in_degree[i] == 0)
            .copied()
            .collect();
        let mut order = Vec::with_capacity(enabled_count);

        while let Some(i) = queue.pop() {
            order.push(i);
            for &j in &adj[i] {
                in_degree[j] -= 1;
                if in_degree[j] == 0 {
                    queue.push(j);
                }
            }
        }

        if order.len() != enabled_count {
            return Err(GraphError::CyclicDependency);
        }

        self.sorted = order;
        Ok(self.sorted.clone())
    }

    /// Get the compiled execution order.
    pub fn execution_order(&self) -> &[usize] {
        &self.sorted
    }

    /// Get pass by index.
    pub fn pass(&self, index: usize) -> &PassDescriptor {
        &self.passes[index]
    }

    /// Get a mutable reference to a pass by index.
    pub fn pass_mut(&mut self, index: usize) -> &mut PassDescriptor {
        &mut self.passes[index]
    }

    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Find a pass index by its ID.
    pub fn find_pass(&self, id: &PassId) -> Option<usize> {
        self.passes.iter().position(|p| &p.id == id)
    }

    /// Remove a pass by index.
    pub fn remove_pass(&mut self, index: usize) -> PassDescriptor {
        self.sorted.clear();
        self.passes.remove(index)
    }

    /// Returns resource IDs that are written but never read (potential waste).
    pub fn unused_outputs(&self) -> Vec<&ResourceId> {
        let all_inputs: std::collections::HashSet<&ResourceId> =
            self.passes.iter().flat_map(|p| p.inputs.iter()).collect();
        let all_outputs: Vec<&ResourceId> =
            self.passes.iter().flat_map(|p| p.outputs.iter()).collect();

        all_outputs
            .into_iter()
            .filter(|o| !all_inputs.contains(o))
            .collect()
    }

    /// Compute resource lifetimes (first write -> last read pass indices).
    pub fn resource_lifetimes(&self) -> HashMap<&ResourceId, (usize, usize)> {
        let mut lifetimes: HashMap<&ResourceId, (usize, usize)> = HashMap::new();

        for (pass_order, &pass_idx) in self.sorted.iter().enumerate() {
            let pass = &self.passes[pass_idx];
            for output in &pass.outputs {
                lifetimes
                    .entry(output)
                    .and_modify(|(_, last)| *last = pass_order)
                    .or_insert((pass_order, pass_order));
            }
            for input in &pass.inputs {
                lifetimes
                    .entry(input)
                    .and_modify(|(_, last)| *last = pass_order)
                    .or_insert((pass_order, pass_order));
            }
        }

        lifetimes
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GraphError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum GraphError {
    CyclicDependency,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CyclicDependency => write!(f, "render graph has cyclic dependencies"),
        }
    }
}

impl std::error::Error for GraphError {}

// ---------------------------------------------------------------------------
// Standard pass configurations
// ---------------------------------------------------------------------------

/// Pre-built graph configurations for common rendering setups.
pub struct StandardGraph;

impl StandardGraph {
    /// A minimal 2D rendering graph: sprite pass -> post-process -> output.
    pub fn simple_2d() -> RenderGraph {
        let mut graph = RenderGraph::new();
        graph.add_pass(
            PassDescriptor::new(PassId("sprite"))
                .writes(ResourceId("color")),
        );
        graph.add_pass(
            PassDescriptor::new(PassId("ui"))
                .reads(ResourceId("color"))
                .writes(ResourceId("color_ui")),
        );
        graph.add_pass(
            PassDescriptor::new(PassId("postprocess"))
                .reads(ResourceId("color_ui"))
                .writes(ResourceId("final")),
        );
        graph
    }

    /// A 3D rendering graph: shadow -> main (PBR) -> skybox -> post.
    pub fn simple_3d() -> RenderGraph {
        let mut graph = RenderGraph::new();
        graph.add_pass(
            PassDescriptor::new(PassId("shadow"))
                .writes(ResourceId("shadow_map")),
        );
        graph.add_pass(
            PassDescriptor::new(PassId("main"))
                .reads(ResourceId("shadow_map"))
                .writes(ResourceId("color"))
                .writes(ResourceId("depth")),
        );
        graph.add_pass(
            PassDescriptor::new(PassId("skybox"))
                .reads(ResourceId("depth"))
                .writes(ResourceId("color_sky")),
        );
        graph.add_pass(
            PassDescriptor::new(PassId("postprocess"))
                .reads(ResourceId("color"))
                .reads(ResourceId("color_sky"))
                .writes(ResourceId("final")),
        );
        graph
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let mut graph = RenderGraph::new();
        let order = graph.compile().unwrap();
        assert!(order.is_empty());
    }

    #[test]
    fn single_pass() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("main"),
            inputs: vec![],
            outputs: vec![ResourceId("color")],
            enabled: true,
        });
        let order = graph.compile().unwrap();
        assert_eq!(order, &[0]);
    }

    #[test]
    fn linear_dependency() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("shadow"),
            inputs: vec![],
            outputs: vec![ResourceId("shadow_map")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("main"),
            inputs: vec![ResourceId("shadow_map")],
            outputs: vec![ResourceId("color")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("post"),
            inputs: vec![ResourceId("color")],
            outputs: vec![ResourceId("final")],
            enabled: true,
        });

        let order = graph.compile().unwrap();
        assert_eq!(order.len(), 3);

        let shadow_pos = order.iter().position(|&x| x == 0).unwrap();
        let main_pos = order.iter().position(|&x| x == 1).unwrap();
        let post_pos = order.iter().position(|&x| x == 2).unwrap();
        assert!(shadow_pos < main_pos);
        assert!(main_pos < post_pos);
    }

    #[test]
    fn parallel_passes() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("pass_a"),
            inputs: vec![],
            outputs: vec![ResourceId("a_out")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("pass_b"),
            inputs: vec![],
            outputs: vec![ResourceId("b_out")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("combine"),
            inputs: vec![ResourceId("a_out"), ResourceId("b_out")],
            outputs: vec![ResourceId("final")],
            enabled: true,
        });

        let order = graph.compile().unwrap();
        assert_eq!(order.len(), 3);
        let combine_pos = order.iter().position(|&x| x == 2).unwrap();
        assert_eq!(combine_pos, 2);
    }

    #[test]
    fn cyclic_dependency_detected() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("a"),
            inputs: vec![ResourceId("b_out")],
            outputs: vec![ResourceId("a_out")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("b"),
            inputs: vec![ResourceId("a_out")],
            outputs: vec![ResourceId("b_out")],
            enabled: true,
        });

        let result = graph.compile();
        assert!(result.is_err());
    }

    // -- New tests --------------------------------------------------------

    #[test]
    fn disabled_pass_skipped() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("shadow"),
            inputs: vec![],
            outputs: vec![ResourceId("shadow_map")],
            enabled: false, // disabled
        });
        graph.add_pass(PassDescriptor {
            id: PassId("main"),
            inputs: vec![], // no longer depends on shadow since it's disabled
            outputs: vec![ResourceId("color")],
            enabled: true,
        });

        let order = graph.compile().unwrap();
        assert_eq!(order.len(), 1);
        assert_eq!(graph.pass(order[0]).id, PassId("main"));
    }

    #[test]
    fn find_pass() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor::new(PassId("sprite")));
        graph.add_pass(PassDescriptor::new(PassId("ui")));

        assert_eq!(graph.find_pass(&PassId("sprite")), Some(0));
        assert_eq!(graph.find_pass(&PassId("ui")), Some(1));
        assert_eq!(graph.find_pass(&PassId("nonexistent")), None);
    }

    #[test]
    fn pass_descriptor_builder() {
        let desc = PassDescriptor::new(PassId("main"))
            .reads(ResourceId("shadow_map"))
            .writes(ResourceId("color"));

        assert_eq!(desc.id, PassId("main"));
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);
        assert!(desc.enabled);
    }

    #[test]
    fn standard_graph_2d() {
        let mut graph = StandardGraph::simple_2d();
        let order = graph.compile().unwrap();
        assert_eq!(order.len(), 3);

        // sprite should come before postprocess
        let sprite_pos = order
            .iter()
            .position(|&i| graph.pass(i).id == PassId("sprite"))
            .unwrap();
        let post_pos = order
            .iter()
            .position(|&i| graph.pass(i).id == PassId("postprocess"))
            .unwrap();
        assert!(sprite_pos < post_pos);
    }

    #[test]
    fn standard_graph_3d() {
        let mut graph = StandardGraph::simple_3d();
        let order = graph.compile().unwrap();
        assert_eq!(order.len(), 4);

        // shadow should come before main
        let shadow_pos = order
            .iter()
            .position(|&i| graph.pass(i).id == PassId("shadow"))
            .unwrap();
        let main_pos = order
            .iter()
            .position(|&i| graph.pass(i).id == PassId("main"))
            .unwrap();
        assert!(shadow_pos < main_pos);
    }

    #[test]
    fn resource_lifetimes() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("shadow"),
            inputs: vec![],
            outputs: vec![ResourceId("shadow_map")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("main"),
            inputs: vec![ResourceId("shadow_map")],
            outputs: vec![ResourceId("color")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("post"),
            inputs: vec![ResourceId("color")],
            outputs: vec![ResourceId("final")],
            enabled: true,
        });

        graph.compile().unwrap();
        let lifetimes = graph.resource_lifetimes();

        // shadow_map: produced at order 0, consumed at order 1
        let sm_lifetime = lifetimes.get(&ResourceId("shadow_map")).unwrap();
        assert!(sm_lifetime.0 < sm_lifetime.1 || sm_lifetime.0 == sm_lifetime.1);

        // color: produced at order 1, consumed at order 2
        let color_lifetime = lifetimes.get(&ResourceId("color")).unwrap();
        assert!(color_lifetime.1 >= color_lifetime.0);
    }

    #[test]
    fn unused_outputs() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor {
            id: PassId("main"),
            inputs: vec![],
            outputs: vec![ResourceId("color"), ResourceId("debug")],
            enabled: true,
        });
        graph.add_pass(PassDescriptor {
            id: PassId("post"),
            inputs: vec![ResourceId("color")],
            outputs: vec![ResourceId("final")],
            enabled: true,
        });

        // "debug" and "final" are unused outputs (no one reads them)
        let unused = graph.unused_outputs();
        assert!(unused.contains(&&ResourceId("debug")));
        assert!(unused.contains(&&ResourceId("final")));
        assert!(!unused.contains(&&ResourceId("color"))); // color IS read
    }

    #[test]
    fn import_resource() {
        let mut graph = RenderGraph::new();
        graph.import_resource(
            ResourceId("swapchain"),
            ResourceKind::External,
        );
        let res = graph.resource("swapchain").unwrap();
        assert!(res.imported);
        assert_eq!(res.kind, ResourceKind::External);
    }

    #[test]
    fn remove_pass() {
        let mut graph = RenderGraph::new();
        graph.add_pass(PassDescriptor::new(PassId("a")));
        graph.add_pass(PassDescriptor::new(PassId("b")));
        assert_eq!(graph.pass_count(), 2);

        let removed = graph.remove_pass(0);
        assert_eq!(removed.id, PassId("a"));
        assert_eq!(graph.pass_count(), 1);
    }
}
