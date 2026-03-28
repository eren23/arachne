pub mod easing;
pub mod tween;
pub mod keyframe;
pub mod clip;
pub mod skeleton;
pub mod skinning;
pub mod animator;
pub mod state_machine;

pub use easing::EasingFunction;
pub use tween::{Lerp, Tween, TweenState, LoopMode, TweenSequence, TweenParallel};
pub use keyframe::{InterpolationMode, Keyframe, KeyframeTrack, AnyKeyframeTrack};
pub use clip::{PropertyPath, AnimationClip, ClipPlayback};
pub use skeleton::Skeleton;
pub use skinning::{
    SkinVertex, SkinningData, compute_joint_matrices, cpu_skin_positions,
    cpu_skin_normals, cpu_skin_mesh, compute_joint_dual_quaternions,
    pack_joint_matrices_f32,
};
pub use animator::{Animator, Crossfade, blend_transforms};
pub use state_machine::{Condition, AnimState, Transition, AnimationStateMachine, BlendNode};
