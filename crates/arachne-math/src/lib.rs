#![cfg_attr(not(feature = "std"), no_std)]

pub mod vec2;
pub mod vec3;
pub mod vec4;
pub mod mat3;
pub mod mat4;
pub mod quat;
pub mod transform;
pub mod rect;
pub mod color;
pub mod random;
pub mod fixed;

pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;
pub use mat3::Mat3;
pub use mat4::Mat4;
pub use quat::Quat;
pub use transform::Transform;
pub use rect::Rect;
pub use color::Color;
pub use random::Rng;
pub use fixed::Fixed;

#[cfg(test)]
mod bench_tests {
    use super::*;
    use core::hint::black_box;

    #[test]
    fn bench_vec3_ops_throughput() {
        let iterations = 1_000_000u64;
        let w = black_box(Vec3::new(0.5, 0.3, 0.1));
        let scale = black_box(0.999f32);
        let mut v0 = black_box(Vec3::new(1.0, 2.0, 3.0));
        let mut v1 = black_box(Vec3::new(4.0, 5.0, 6.0));
        let mut v2 = black_box(Vec3::new(7.0, 8.0, 9.0));
        let mut v3 = black_box(Vec3::new(0.1, 0.2, 0.4));

        let start = std::time::Instant::now();
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

        let ops = iterations * 4 * 3; // 4 streams, 3 ops each
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

    #[test]
    fn bench_mat4_multiply_throughput() {
        let iterations = 1_000_000u64;
        let a = Mat4::from_rotation_z(0.1);
        let b = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let mut result = Mat4::IDENTITY;

        let start = std::time::Instant::now();
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

    #[test]
    fn f64_reference_accuracy() {
        // Vec3 ops match f64 reference within 1e-5 relative error
        let a = Vec3::new(1.23456, 7.89012, 3.45678);
        let b = Vec3::new(9.87654, 5.43210, 1.09876);

        let dot_f32 = a.dot(b) as f64;
        let dot_f64 = 1.23456_f64 * 9.87654 + 7.89012_f64 * 5.43210 + 3.45678_f64 * 1.09876;
        let rel_err = ((dot_f32 - dot_f64) / dot_f64).abs();
        assert!(
            rel_err < 1e-5,
            "dot product relative error {rel_err} exceeds 1e-5"
        );

        let cross = a.cross(b);
        let cx = (7.89012_f64 * 1.09876 - 3.45678_f64 * 5.43210) as f32;
        let cy = (3.45678_f64 * 9.87654 - 1.23456_f64 * 1.09876) as f32;
        let cz = (1.23456_f64 * 5.43210 - 7.89012_f64 * 9.87654) as f32;
        assert!((cross.x - cx).abs() < 1e-3);
        assert!((cross.y - cy).abs() < 1e-3);
        assert!((cross.z - cz).abs() < 1e-3);

        // Normalize
        let len_f64 = (1.23456_f64 * 1.23456 + 7.89012_f64 * 7.89012 + 3.45678_f64 * 3.45678).sqrt();
        let norm = a.normalize();
        let norm_x_f64 = 1.23456_f64 / len_f64;
        let rel_err = ((norm.x as f64 - norm_x_f64) / norm_x_f64).abs();
        assert!(
            rel_err < 1e-5,
            "normalize x relative error {rel_err} exceeds 1e-5"
        );
    }
}
