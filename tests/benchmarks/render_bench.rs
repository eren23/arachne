//! Benchmark tests: Render operations.
//!
//! - Sprite batch setup for 100K sprites (Batcher sort + merge)
//! - Draw call merging ratio

use arachne_render::{Batcher, TextureHandle};
use arachne_render::render2d::batch::{DrawCommand, SortKey};
use std::hint::black_box;
use std::time::Instant;

/// Creates a sprite draw command matching the instanced rendering pattern used
/// by SpriteRenderer: all sprites share the same quad geometry (vertex 0..4,
/// index 0..6) and differ only in instance_offset.
fn make_instanced_sprite_command(texture_id: u32, depth: f32, instance_idx: u32) -> DrawCommand {
    DrawCommand {
        sort_key: SortKey {
            shader_id: 0,
            texture: TextureHandle(texture_id),
            depth,
        },
        // Shared quad geometry for all sprites.
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        // Per-sprite instance data.
        instance_offset: instance_idx,
        instance_count: 1,
        base_vertex: 0,
    }
}

/// Batch setup for 100K instanced sprites using the Batcher.
/// Measures sort_and_merge time for 100K draw commands across 8 textures.
#[test]
fn bench_sprite_batch_100k() {
    let sprite_count = 100_000u32;
    let num_textures = 8u32;
    let per_tex = sprite_count / num_textures;

    let mut batcher = Batcher::new();
    // Emit commands grouped by texture with contiguous instance ranges
    // (models what a SpriteRenderer produces after sorting by texture).
    for tex_id in 0..num_textures {
        for i in 0..per_tex {
            let instance_idx = tex_id * per_tex + i;
            batcher.add(make_instanced_sprite_command(tex_id, 0.0, instance_idx));
        }
    }

    assert_eq!(batcher.command_count(), sprite_count as usize);

    let start = Instant::now();
    let (merged, stats) = batcher.sort_and_merge();
    let elapsed = start.elapsed();
    let _ = black_box(&merged);

    eprintln!(
        "Batcher sort+merge {}K sprites: {:.2}ms, {} draw calls (from {} commands)",
        sprite_count / 1000,
        elapsed.as_secs_f64() * 1000.0,
        stats.draw_calls,
        stats.total_commands
    );

    // Sorting 100K commands should complete in well under 100ms.
    assert!(
        elapsed.as_secs_f64() < 0.1,
        "Batcher sort+merge took {:.2}ms, expected <100ms",
        elapsed.as_secs_f64() * 1000.0
    );

    // With 8 textures, we expect exactly 8 draw calls after merging.
    assert!(
        merged.len() <= num_textures as usize,
        "Expected <= {} draw calls, got {}",
        num_textures,
        merged.len()
    );
}

/// Draw call merging ratio: 1000 sprites across 4 textures at same depth.
/// With 4 textures and contiguous instance ranges, each texture should merge
/// to one draw call.
#[test]
fn bench_draw_call_merging_ratio() {
    let sprite_count = 1000u32;
    let num_textures = 4u32;
    let per_tex = sprite_count / num_textures;

    let mut batcher = Batcher::new();
    for tex_id in 0..num_textures {
        for i in 0..per_tex {
            let instance_idx = tex_id * per_tex + i;
            batcher.add(make_instanced_sprite_command(tex_id, 0.0, instance_idx));
        }
    }

    let (merged, stats) = batcher.sort_and_merge();

    let reduction = stats.reduction_ratio();
    eprintln!(
        "Merging ratio: {} commands -> {} draw calls (reduction {:.1}%)",
        stats.total_commands,
        stats.draw_calls,
        reduction * 100.0
    );

    // With 4 textures, sorted by texture, we should get exactly 4 draw calls.
    assert!(
        merged.len() <= 4,
        "Expected <= 4 draw calls for 4 textures, got {}",
        merged.len()
    );
    assert!(
        reduction >= 0.99,
        "Draw call reduction {:.1}% is below 99% threshold",
        reduction * 100.0
    );
}

/// Batch setup for 10K sprites with a single texture: should merge to 1 draw
/// call since they share geometry and have contiguous instance offsets.
#[test]
fn bench_sprite_batch_single_texture() {
    let sprite_count = 10_000u32;

    let mut batcher = Batcher::new();
    for i in 0..sprite_count {
        batcher.add(make_instanced_sprite_command(0, 0.0, i));
    }

    let start = Instant::now();
    let (merged, stats) = batcher.sort_and_merge();
    let elapsed = start.elapsed();

    eprintln!(
        "Single texture {}K sprites: {:.2}ms, {} draw calls",
        sprite_count / 1000,
        elapsed.as_secs_f64() * 1000.0,
        merged.len()
    );

    assert_eq!(
        merged.len(),
        1,
        "All sprites with same texture should merge to 1 draw call"
    );
    assert!(
        elapsed.as_secs_f64() < 0.01,
        "Batch merge took {:.2}ms, expected <10ms for 10K sprites",
        elapsed.as_secs_f64() * 1000.0
    );
    let _ = black_box(stats);
}
