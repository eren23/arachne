use arachne_math::{Mat4, Transform};

// SKELETON ------

pub const MAX_BONES: usize = 128;

#[derive(Clone, Debug)]
pub struct Skeleton {
    pub bone_names: Vec<String>,
    pub parent_indices: Vec<Option<usize>>,
    pub bind_pose: Vec<Transform>,
    pub inverse_bind_matrices: Vec<Mat4>,
}

impl Skeleton {
    pub fn new(
        names: Vec<String>,
        parents: Vec<Option<usize>>,
        bind_pose: Vec<Transform>,
    ) -> Self {
        assert_eq!(names.len(), parents.len());
        assert_eq!(names.len(), bind_pose.len());
        assert!(
            names.len() <= MAX_BONES,
            "skeleton has {} bones, max is {}",
            names.len(),
            MAX_BONES
        );

        let inverse_bind_matrices = Self::compute_inverse_bind_matrices(&bind_pose, &parents);

        Self {
            bone_names: names,
            parent_indices: parents,
            bind_pose,
            inverse_bind_matrices,
        }
    }

    #[inline]
    pub fn bone_count(&self) -> usize {
        self.bone_names.len()
    }

    #[inline]
    pub fn find_bone(&self, name: &str) -> Option<usize> {
        self.bone_names.iter().position(|n| n == name)
    }

    pub fn compute_global_transforms(
        local_poses: &[Transform],
        parents: &[Option<usize>],
    ) -> Vec<Mat4> {
        let count = local_poses.len();
        let mut globals: Vec<Mat4> = Vec::with_capacity(count);

        for i in 0..count {
            let local = local_poses[i].local_to_world();
            let global = match parents[i] {
                Some(parent_idx) => globals[parent_idx].mul_mat4(local),
                None => local,
            };
            globals.push(global);
        }

        globals
    }

    pub fn compute_inverse_bind_matrices(
        bind_pose: &[Transform],
        parents: &[Option<usize>],
    ) -> Vec<Mat4> {
        let globals = Self::compute_global_transforms(bind_pose, parents);
        globals
            .iter()
            .map(|g| g.inverse().unwrap_or(Mat4::IDENTITY))
            .collect()
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::Vec3;
    use core::hint::black_box;

    const EPSILON: f32 = 1e-4;

    fn approx_eq_f32(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn three_bone_chain_globals() {
        let names = vec!["root".into(), "bone1".into(), "bone2".into()];
        let parents = vec![None, Some(0), Some(1)];
        let bind_pose = vec![
            Transform::IDENTITY,
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
            Transform::from_position(Vec3::new(2.0, 0.0, 0.0)),
        ];

        let skeleton = Skeleton::new(names, parents, bind_pose.clone());
        assert_eq!(skeleton.bone_count(), 3);

        let globals = Skeleton::compute_global_transforms(&bind_pose, &skeleton.parent_indices);

        // root at origin
        assert!(approx_eq_f32(globals[0].cols[3][0], 0.0));
        assert!(approx_eq_f32(globals[0].cols[3][1], 0.0));
        assert!(approx_eq_f32(globals[0].cols[3][2], 0.0));

        // bone1 at (1,0,0)
        assert!(approx_eq_f32(globals[1].cols[3][0], 1.0));
        assert!(approx_eq_f32(globals[1].cols[3][1], 0.0));
        assert!(approx_eq_f32(globals[1].cols[3][2], 0.0));

        // bone2 at (3,0,0) = bone1(1,0,0) + local(2,0,0)
        assert!(
            approx_eq_f32(globals[2].cols[3][0], 3.0),
            "bone2 x should be 3.0, got {}",
            globals[2].cols[3][0]
        );
        assert!(approx_eq_f32(globals[2].cols[3][1], 0.0));
        assert!(approx_eq_f32(globals[2].cols[3][2], 0.0));
    }

    #[test]
    fn find_bone_by_name() {
        let names = vec!["root".into(), "spine".into(), "head".into()];
        let parents = vec![None, Some(0), Some(1)];
        let bind_pose = vec![
            Transform::IDENTITY,
            Transform::IDENTITY,
            Transform::IDENTITY,
        ];

        let skeleton = Skeleton::new(names, parents, bind_pose);
        assert_eq!(skeleton.find_bone("spine"), Some(1));
        assert_eq!(skeleton.find_bone("head"), Some(2));
        assert_eq!(skeleton.find_bone("arm"), None);
    }

    #[test]
    fn max_bones_validation() {
        let count = MAX_BONES;
        let names: Vec<String> = (0..count).map(|i| format!("bone_{i}")).collect();
        let parents: Vec<Option<usize>> = (0..count)
            .map(|i| if i == 0 { None } else { Some(0) })
            .collect();
        let bind_pose = vec![Transform::IDENTITY; count];

        let skeleton = Skeleton::new(names, parents, bind_pose);
        assert_eq!(skeleton.bone_count(), MAX_BONES);
    }

    #[test]
    #[should_panic]
    fn exceeds_max_bones_panics() {
        let count = MAX_BONES + 1;
        let names: Vec<String> = (0..count).map(|i| format!("bone_{i}")).collect();
        let parents: Vec<Option<usize>> = (0..count)
            .map(|i| if i == 0 { None } else { Some(0) })
            .collect();
        let bind_pose = vec![Transform::IDENTITY; count];

        Skeleton::new(names, parents, bind_pose);
    }

    #[test]
    fn bench_100_bone_joint_matrices() {
        let count = 100;
        let names: Vec<String> = (0..count).map(|i| format!("bone_{i}")).collect();
        let parents: Vec<Option<usize>> = (0..count)
            .map(|i| if i == 0 { None } else { Some(i - 1) })
            .collect();
        let bind_pose: Vec<Transform> = (0..count)
            .map(|_| Transform::from_position(Vec3::new(0.1, 0.0, 0.0)))
            .collect();

        let _skeleton = Skeleton::new(names, parents.clone(), bind_pose.clone());

        let iterations = 1000u32;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let globals = black_box(
                Skeleton::compute_global_transforms(
                    black_box(&bind_pose),
                    black_box(&parents),
                ),
            );
            black_box(globals);
        }
        let elapsed = start.elapsed();
        let per_iter_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
        eprintln!(
            "100-bone global transforms: {:.4}ms per iteration ({} iterations in {:.3}ms)",
            per_iter_ms,
            iterations,
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            per_iter_ms < 0.5,
            "100-bone transforms took {per_iter_ms:.4}ms, should be < 0.5ms"
        );
    }
}
