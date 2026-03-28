use crate::texture::TextureHandle;

/// A single draw command before batching.
#[derive(Clone, Debug)]
pub struct DrawCommand {
    pub sort_key: SortKey,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub instance_offset: u32,
    pub instance_count: u32,
    pub base_vertex: i32,
}

/// Sorting key: (shader_id, texture, depth).
/// Lower values are drawn first.
#[derive(Clone, Debug, PartialEq)]
pub struct SortKey {
    pub shader_id: u32,
    pub texture: TextureHandle,
    pub depth: f32,
}

impl SortKey {
    fn sort_tuple(&self) -> (u32, u32, u32) {
        (
            self.shader_id,
            self.texture.0,
            self.depth.to_bits(),
        )
    }
}

impl Eq for SortKey {}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_tuple().cmp(&other.sort_tuple())
    }
}

/// A merged draw call combining consecutive commands with the same state.
#[derive(Clone, Debug)]
pub struct MergedDrawCall {
    pub shader_id: u32,
    pub texture: TextureHandle,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub instance_offset: u32,
    pub instance_count: u32,
    pub base_vertex: i32,
}

/// Draw call statistics.
#[derive(Clone, Debug, Default)]
pub struct BatchStats {
    pub total_commands: u32,
    pub merged_commands: u32,
    pub draw_calls: u32,
}

impl BatchStats {
    pub fn reduction_ratio(&self) -> f64 {
        if self.total_commands == 0 {
            return 1.0;
        }
        1.0 - (self.draw_calls as f64 / self.total_commands as f64)
    }
}

/// Collects draw commands, sorts by state, and merges into minimal draw calls.
pub struct Batcher {
    commands: Vec<DrawCommand>,
}

impl Batcher {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn add(&mut self, command: DrawCommand) {
        self.commands.push(command);
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Sort by (shader, texture, depth) and merge consecutive same-state commands.
    pub fn sort_and_merge(&mut self) -> (Vec<MergedDrawCall>, BatchStats) {
        let total = self.commands.len() as u32;

        if self.commands.is_empty() {
            return (
                Vec::new(),
                BatchStats {
                    total_commands: 0,
                    merged_commands: 0,
                    draw_calls: 0,
                },
            );
        }

        // Sort by sort key
        self.commands.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));

        let mut merged: Vec<MergedDrawCall> = Vec::new();
        let mut merged_count = 0u32;

        for cmd in &self.commands {
            let can_merge = merged.last().map_or(false, |last: &MergedDrawCall| {
                last.shader_id == cmd.sort_key.shader_id
                    && last.texture == cmd.sort_key.texture
                    && last.base_vertex == cmd.base_vertex
                    // For instanced draws, merge if contiguous instances
                    && (cmd.instance_count > 0
                        && last.instance_offset + last.instance_count == cmd.instance_offset
                        && last.vertex_offset == cmd.vertex_offset
                        && last.vertex_count == cmd.vertex_count
                        && last.index_offset == cmd.index_offset
                        && last.index_count == cmd.index_count)
                    // For non-instanced draws, merge if contiguous vertices/indices
                    || (cmd.instance_count == 0
                        && last.shader_id == cmd.sort_key.shader_id
                        && last.texture == cmd.sort_key.texture
                        && cmd.index_count > 0
                        && last.index_offset + last.index_count == cmd.index_offset)
                    || (cmd.instance_count == 0
                        && cmd.index_count == 0
                        && last.shader_id == cmd.sort_key.shader_id
                        && last.texture == cmd.sort_key.texture
                        && last.vertex_offset + last.vertex_count == cmd.vertex_offset)
            });

            if can_merge {
                let last = merged.last_mut().unwrap();
                if cmd.instance_count > 0 {
                    last.instance_count += cmd.instance_count;
                } else if cmd.index_count > 0 {
                    last.index_count += cmd.index_count;
                } else {
                    last.vertex_count += cmd.vertex_count;
                }
                merged_count += 1;
            } else {
                merged.push(MergedDrawCall {
                    shader_id: cmd.sort_key.shader_id,
                    texture: cmd.sort_key.texture,
                    vertex_offset: cmd.vertex_offset,
                    vertex_count: cmd.vertex_count,
                    index_offset: cmd.index_offset,
                    index_count: cmd.index_count,
                    instance_offset: cmd.instance_offset,
                    instance_count: cmd.instance_count,
                    base_vertex: cmd.base_vertex,
                });
            }
        }

        let stats = BatchStats {
            total_commands: total,
            merged_commands: merged_count,
            draw_calls: merged.len() as u32,
        };

        (merged, stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_batcher() {
        let mut batcher = Batcher::new();
        let (calls, stats) = batcher.sort_and_merge();
        assert!(calls.is_empty());
        assert_eq!(stats.total_commands, 0);
        assert_eq!(stats.draw_calls, 0);
    }

    #[test]
    fn single_command() {
        let mut batcher = Batcher::new();
        batcher.add(DrawCommand {
            sort_key: SortKey {
                shader_id: 0,
                texture: TextureHandle(0),
                depth: 0.0,
            },
            vertex_offset: 0,
            vertex_count: 4,
            index_offset: 0,
            index_count: 6,
            instance_offset: 0,
            instance_count: 0,
            base_vertex: 0,
        });
        let (calls, stats) = batcher.sort_and_merge();
        assert_eq!(calls.len(), 1);
        assert_eq!(stats.total_commands, 1);
        assert_eq!(stats.draw_calls, 1);
    }

    #[test]
    fn merge_same_state_instanced() {
        let mut batcher = Batcher::new();
        let tex = TextureHandle(0);

        // 100 sprites with same texture, contiguous instances
        for i in 0..100 {
            batcher.add(DrawCommand {
                sort_key: SortKey {
                    shader_id: 0,
                    texture: tex,
                    depth: 0.0,
                },
                vertex_offset: 0,
                vertex_count: 4,
                index_offset: 0,
                index_count: 6,
                instance_offset: i,
                instance_count: 1,
                base_vertex: 0,
            });
        }

        let (calls, stats) = batcher.sort_and_merge();
        assert_eq!(calls.len(), 1, "100 same-texture sprites should merge to 1 draw call");
        assert_eq!(calls[0].instance_count, 100);
        assert_eq!(stats.total_commands, 100);
        assert_eq!(stats.draw_calls, 1);
    }

    #[test]
    fn separate_textures_not_merged() {
        let mut batcher = Batcher::new();

        // 100 sprites with 4 different textures, contiguous instances per texture
        for tex_idx in 0..4u32 {
            for i in 0..25u32 {
                let instance_idx = tex_idx * 25 + i;
                batcher.add(DrawCommand {
                    sort_key: SortKey {
                        shader_id: 0,
                        texture: TextureHandle(tex_idx),
                        depth: 0.0,
                    },
                    vertex_offset: 0,
                    vertex_count: 4,
                    index_offset: 0,
                    index_count: 6,
                    instance_offset: instance_idx,
                    instance_count: 1,
                    base_vertex: 0,
                });
            }
        }

        let (calls, stats) = batcher.sort_and_merge();
        assert_eq!(calls.len(), 4, "4 textures -> 4 draw calls");
        assert_eq!(stats.total_commands, 100);
        assert_eq!(stats.draw_calls, 4);
    }

    #[test]
    fn depth_sorting() {
        let mut batcher = Batcher::new();
        let tex = TextureHandle(0);

        // Add commands in reverse depth order
        for i in (0..10).rev() {
            batcher.add(DrawCommand {
                sort_key: SortKey {
                    shader_id: 0,
                    texture: tex,
                    depth: i as f32,
                },
                vertex_offset: i as u32 * 4,
                vertex_count: 4,
                index_offset: 0,
                index_count: 0,
                instance_offset: 0,
                instance_count: 0,
                base_vertex: 0,
            });
        }

        let (calls, _stats) = batcher.sort_and_merge();
        // Should be sorted by depth (ascending via bit comparison)
        for i in 1..calls.len() {
            assert!(
                calls[i - 1].vertex_offset <= calls[i].vertex_offset
                    || calls[i - 1].shader_id < calls[i].shader_id
                    || calls[i - 1].texture.0 < calls[i].texture.0,
                "draw calls should be sorted"
            );
        }
    }

    #[test]
    fn large_batch_reduction() {
        let mut batcher = Batcher::new();

        // 1000 sprites with 4 textures, contiguous instance ranges per texture
        // (models what SpriteRenderer produces after sorting)
        let per_tex = 250u32;
        for tex_idx in 0..4u32 {
            for i in 0..per_tex {
                let instance_base = tex_idx * per_tex;
                batcher.add(DrawCommand {
                    sort_key: SortKey {
                        shader_id: 0,
                        texture: TextureHandle(tex_idx),
                        depth: 0.0,
                    },
                    vertex_offset: 0,
                    vertex_count: 4,
                    index_offset: 0,
                    index_count: 6,
                    instance_offset: instance_base + i,
                    instance_count: 1,
                    base_vertex: 0,
                });
            }
        }

        let (calls, stats) = batcher.sort_and_merge();
        assert!(
            calls.len() <= 10,
            "1000 sprites/4 textures should produce <= 10 draw calls, got {}",
            calls.len()
        );
        assert_eq!(stats.total_commands, 1000);
        assert!(
            stats.reduction_ratio() >= 0.9,
            "reduction ratio should be >= 90%, got {:.1}%",
            stats.reduction_ratio() * 100.0
        );
    }
}
