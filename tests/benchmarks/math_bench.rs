//! Benchmark tests: Math operations throughput.
//!
//! - Vec3 ops throughput (target: >=200M ops/sec)
//! - Mat4 multiply throughput (target: >=50M ops/sec)
//! - Quaternion slerp throughput

use arachne_math::{Mat4, Quat, Vec3};
use std::hint::black_box;
use std::time::Instant;

/// Vec3 add + scale + normalize throughput.
/// Target: >= 200M ops/sec.
#[test]
fn bench_vec3_ops_throughput() {
    let iterations = 1_000_000u64;
    let w = black_box(Vec3::new(0.5, 0.3, 0.1));
    let scale = black_box(0.999f32);
    let mut v0 = black_box(Vec3::new(1.0, 2.0, 3.0));
    let mut v1 = black_box(Vec3::new(4.0, 5.0, 6.0));
    let mut v2 = black_box(Vec3::new(7.0, 8.0, 9.0));
    let mut v3 = black_box(Vec3::new(0.1, 0.2, 0.4));

    let start = Instant::now();
    for _ in 0..iterations {
        v0 = (v0 + w) * scale;
        v1 = (v1 + w) * scale;
        v2 = (v2 + w) * scale;
        v3 = (v3 + w) * scale;
        v0 = v0.normalize();
        v1 = v1.normalize();
        v2 = v2.normalize();
        v3 = v3.normalize();
    }
    let elapsed = start.elapsed();
    black_box((v0, v1, v2, v3));

    // 4 streams, 3 ops each (add, mul, normalize).
    let ops = iterations * 4 * 3;
    let ops_per_sec = ops as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Vec3 ops: {:.0}M ops/sec ({} iterations in {:.3}ms)",
        ops_per_sec / 1_000_000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(
        ops_per_sec >= 200_000_000.0,
        "Vec3 ops throughput {:.0}M ops/sec is below 200M ops/sec threshold",
        ops_per_sec / 1_000_000.0
    );
}

/// Mat4 multiply throughput.
/// Target: >= 50M ops/sec.
#[test]
fn bench_mat4_multiply_throughput() {
    let iterations = 1_000_000u64;
    let a = Mat4::from_rotation_z(0.1);
    let b = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let mut result = Mat4::IDENTITY;

    let start = Instant::now();
    for _ in 0..iterations {
        result = black_box(a) * black_box(b);
    }
    let elapsed = start.elapsed();
    let _ = black_box(result);

    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Mat4 multiply: {:.0}M ops/sec ({} iterations in {:.3}ms)",
        ops_per_sec / 1_000_000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(
        ops_per_sec >= 50_000_000.0,
        "Mat4 multiply throughput {:.0}M ops/sec is below 50M ops/sec threshold",
        ops_per_sec / 1_000_000.0
    );
}

/// Quaternion slerp throughput.
#[test]
fn bench_quat_slerp_throughput() {
    let iterations = 1_000_000u64;
    let q1 = black_box(Quat::from_axis_angle(Vec3::Y, 0.0));
    let q2 = black_box(Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_2));
    let mut result = Quat::IDENTITY;

    let start = Instant::now();
    for i in 0..iterations {
        let t = (i as f32 / iterations as f32).fract();
        result = q1.slerp(q2, black_box(t));
    }
    let elapsed = start.elapsed();
    let _ = black_box(result);

    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Quat slerp: {:.0}M ops/sec ({} iterations in {:.3}ms)",
        ops_per_sec / 1_000_000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0
    );
    // Slerp involves trig -- 20M ops/sec is a reasonable floor.
    assert!(
        ops_per_sec >= 20_000_000.0,
        "Quat slerp throughput {:.0}M ops/sec is below 20M ops/sec threshold",
        ops_per_sec / 1_000_000.0
    );
}

/// Vec3 dot product throughput.
#[test]
fn bench_vec3_dot_throughput() {
    let iterations = 2_000_000u64;
    let a = black_box(Vec3::new(1.0, 2.0, 3.0));
    let b = black_box(Vec3::new(4.0, 5.0, 6.0));
    let mut sum = 0.0f32;

    let start = Instant::now();
    for _ in 0..iterations {
        sum += a.dot(b);
    }
    let elapsed = start.elapsed();
    let _ = black_box(sum);

    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Vec3 dot: {:.0}M ops/sec ({} iterations in {:.3}ms)",
        ops_per_sec / 1_000_000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(
        ops_per_sec >= 200_000_000.0,
        "Vec3 dot throughput {:.0}M ops/sec is below 200M ops/sec threshold",
        ops_per_sec / 1_000_000.0
    );
}

/// Vec3 cross product throughput.
#[test]
fn bench_vec3_cross_throughput() {
    let iterations = 2_000_000u64;
    let mut a = black_box(Vec3::new(1.0, 2.0, 3.0));
    let b = black_box(Vec3::new(4.0, 5.0, 6.0));

    let start = Instant::now();
    for _ in 0..iterations {
        a = a.cross(b);
    }
    let elapsed = start.elapsed();
    let _ = black_box(a);

    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Vec3 cross: {:.0}M ops/sec ({} iterations in {:.3}ms)",
        ops_per_sec / 1_000_000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(
        ops_per_sec >= 200_000_000.0,
        "Vec3 cross throughput {:.0}M ops/sec is below 200M ops/sec threshold",
        ops_per_sec / 1_000_000.0
    );
}
