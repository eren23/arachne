use std::collections::{HashMap, HashSet};

use crate::animator::Crossfade;

// ---------------------------------------------------------------------------
// Condition – evaluated to decide if a transition fires
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Condition {
    BoolParam { name: String, value: bool },
    FloatGreaterThan { name: String, threshold: f32 },
    FloatLessThan { name: String, threshold: f32 },
    Trigger { name: String },
    /// All sub-conditions must be true.
    And(Vec<Condition>),
    /// At least one sub-condition must be true.
    Or(Vec<Condition>),
    /// The current animation has finished playing.
    AnimationFinished,
    /// Elapsed time in the current state exceeds this threshold.
    TimeInState(f32),
}

// ---------------------------------------------------------------------------
// AnimState
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct AnimState {
    pub name: String,
    pub clip_index: usize,
    pub speed: f32,
    /// Whether this state loops or plays once.
    pub looping: bool,
}

impl AnimState {
    #[inline]
    pub fn new(name: &str, clip_index: usize, speed: f32) -> Self {
        Self {
            name: name.to_string(),
            clip_index,
            speed,
            looping: true,
        }
    }

    pub fn once(name: &str, clip_index: usize, speed: f32) -> Self {
        Self {
            name: name.to_string(),
            clip_index,
            speed,
            looping: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Transition {
    pub from: usize,
    pub to: usize,
    pub condition: Condition,
    pub blend_duration: f32,
    /// Priority: higher priority transitions are evaluated first.
    pub priority: i32,
}

impl Transition {
    pub fn new(from: usize, to: usize, condition: Condition, blend_duration: f32) -> Self {
        Self {
            from,
            to,
            condition,
            blend_duration,
            priority: 0,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

// ---------------------------------------------------------------------------
// BlendNode – tree-based blending
// ---------------------------------------------------------------------------

/// A node in a blend tree that produces an animation output.
#[derive(Clone, Debug)]
pub enum BlendNode {
    /// Plays a single clip.
    Clip { clip_index: usize, speed: f32 },
    /// Linear blend between two child nodes based on a parameter.
    Blend1D {
        param_name: String,
        children: Vec<(f32, BlendNode)>,
    },
    /// Additive blend: base + additive * weight.
    Additive {
        base: Box<BlendNode>,
        additive: Box<BlendNode>,
        weight_param: String,
    },
}

impl BlendNode {
    /// Evaluate the blend tree, returning a list of (clip_index, weight) pairs
    /// that should be mixed together.
    pub fn evaluate(&self, params: &HashMap<String, f32>) -> Vec<(usize, f32)> {
        match self {
            BlendNode::Clip { clip_index, .. } => {
                vec![(*clip_index, 1.0)]
            }
            BlendNode::Blend1D {
                param_name,
                children,
            } => {
                let param_val = params.get(param_name).copied().unwrap_or(0.0);

                if children.is_empty() {
                    return Vec::new();
                }
                if children.len() == 1 {
                    return children[0].1.evaluate(params);
                }

                // Find the two nodes to blend between
                // Children are assumed to be sorted by their threshold values
                let mut low_idx = 0;
                let mut high_idx = 1;

                for i in 0..children.len() - 1 {
                    if param_val >= children[i].0 && param_val <= children[i + 1].0 {
                        low_idx = i;
                        high_idx = i + 1;
                        break;
                    }
                    if i == children.len() - 2 {
                        // Clamp to ends
                        if param_val < children[0].0 {
                            return children[0].1.evaluate(params);
                        } else {
                            return children[children.len() - 1].1.evaluate(params);
                        }
                    }
                }

                let range = children[high_idx].0 - children[low_idx].0;
                let t = if range > 0.0 {
                    (param_val - children[low_idx].0) / range
                } else {
                    0.0
                };

                let low_clips = children[low_idx].1.evaluate(params);
                let high_clips = children[high_idx].1.evaluate(params);

                let mut result = Vec::new();
                for (clip, weight) in low_clips {
                    result.push((clip, weight * (1.0 - t)));
                }
                for (clip, weight) in high_clips {
                    result.push((clip, weight * t));
                }
                result
            }
            BlendNode::Additive {
                base,
                additive,
                weight_param,
            } => {
                let weight = params.get(weight_param).copied().unwrap_or(0.0);
                let mut result = base.evaluate(params);
                for (clip, w) in additive.evaluate(params) {
                    result.push((clip, w * weight));
                }
                result
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AnimationStateMachine
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct AnimationStateMachine {
    pub states: Vec<AnimState>,
    pub transitions: Vec<Transition>,
    pub current_state: usize,
    pub params_bool: HashMap<String, bool>,
    pub params_float: HashMap<String, f32>,
    pub triggers: HashSet<String>,
    pub active_crossfade: Option<Crossfade>,
    /// Time spent in the current state (seconds).
    pub time_in_state: f32,
    /// Whether the current animation has finished (only meaningful for non-looping).
    pub animation_finished: bool,
    /// Optional blend trees per state (state index -> blend node).
    pub blend_trees: HashMap<usize, BlendNode>,
}

impl AnimationStateMachine {
    #[inline]
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
            transitions: Vec::new(),
            current_state: 0,
            params_bool: HashMap::new(),
            params_float: HashMap::new(),
            triggers: HashSet::new(),
            active_crossfade: None,
            time_in_state: 0.0,
            animation_finished: false,
            blend_trees: HashMap::new(),
        }
    }

    #[inline]
    pub fn add_state(&mut self, state: AnimState) -> usize {
        let idx = self.states.len();
        self.states.push(state);
        idx
    }

    #[inline]
    pub fn add_transition(&mut self, transition: Transition) {
        self.transitions.push(transition);
    }

    /// Attach a blend tree to a state.
    pub fn set_blend_tree(&mut self, state_index: usize, tree: BlendNode) {
        self.blend_trees.insert(state_index, tree);
    }

    #[inline]
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.params_bool.insert(name.to_string(), value);
    }

    #[inline]
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.params_bool.get(name).copied()
    }

    #[inline]
    pub fn set_float(&mut self, name: &str, value: f32) {
        self.params_float.insert(name.to_string(), value);
    }

    #[inline]
    pub fn get_float(&self, name: &str) -> Option<f32> {
        self.params_float.get(name).copied()
    }

    #[inline]
    pub fn set_trigger(&mut self, name: &str) {
        self.triggers.insert(name.to_string());
    }

    /// Signal that the current animation has finished playing.
    pub fn notify_animation_finished(&mut self) {
        self.animation_finished = true;
    }

    /// Force transition to a specific state (bypassing conditions).
    pub fn force_state(&mut self, state_index: usize) {
        self.current_state = state_index;
        self.time_in_state = 0.0;
        self.animation_finished = false;
        self.active_crossfade = None;
    }

    fn evaluate_condition(&self, condition: &Condition) -> bool {
        match condition {
            Condition::BoolParam { name, value } => {
                self.params_bool.get(name).copied() == Some(*value)
            }
            Condition::FloatGreaterThan { name, threshold } => self
                .params_float
                .get(name)
                .copied()
                .map(|v| v > *threshold)
                .unwrap_or(false),
            Condition::FloatLessThan { name, threshold } => self
                .params_float
                .get(name)
                .copied()
                .map(|v| v < *threshold)
                .unwrap_or(false),
            Condition::Trigger { name } => self.triggers.contains(name),
            Condition::And(conditions) => conditions.iter().all(|c| self.evaluate_condition(c)),
            Condition::Or(conditions) => conditions.iter().any(|c| self.evaluate_condition(c)),
            Condition::AnimationFinished => self.animation_finished,
            Condition::TimeInState(threshold) => self.time_in_state >= *threshold,
        }
    }

    pub fn evaluate(&mut self, dt: f32) {
        self.time_in_state += dt;

        // If there is an active crossfade, advance it
        if let Some(ref mut cf) = self.active_crossfade {
            cf.update(dt);
            if cf.is_complete() {
                self.current_state = cf.to_clip;
                self.active_crossfade = None;
                self.time_in_state = 0.0;
                self.animation_finished = false;
            }
            return;
        }

        // Check transitions from current state, sorted by priority
        let mut candidates: Vec<(usize, i32)> = self
            .transitions
            .iter()
            .enumerate()
            .filter(|(_, t)| t.from == self.current_state)
            .map(|(i, t)| (i, t.priority))
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1)); // highest priority first

        let mut triggered_transition: Option<usize> = None;
        let mut consumed_trigger: Option<String> = None;

        for (t_idx, _) in candidates {
            let t = &self.transitions[t_idx];
            let condition_met = self.evaluate_condition(&t.condition);

            if condition_met {
                triggered_transition = Some(t_idx);
                // Check if the outermost condition is a trigger to consume
                if let Condition::Trigger { name } = &t.condition {
                    consumed_trigger = Some(name.clone());
                }
                break;
            }
        }

        // Consume trigger
        if let Some(name) = consumed_trigger {
            self.triggers.remove(&name);
        }

        // Start transition
        if let Some(t_idx) = triggered_transition {
            let t = &self.transitions[t_idx];
            let to_state = t.to;
            let blend_duration = t.blend_duration;

            if blend_duration > 0.0 {
                self.active_crossfade =
                    Some(Crossfade::new(self.current_state, to_state, blend_duration));
            } else {
                self.current_state = to_state;
                self.time_in_state = 0.0;
                self.animation_finished = false;
            }
        }
    }

    #[inline]
    pub fn current_state(&self) -> &AnimState {
        &self.states[self.current_state]
    }

    #[inline]
    pub fn current_state_index(&self) -> usize {
        self.current_state
    }

    #[inline]
    pub fn blend_weight(&self) -> Option<(usize, usize, f32)> {
        self.active_crossfade.as_ref().map(|cf| {
            let weight = if cf.duration > 0.0 {
                cf.elapsed / cf.duration
            } else {
                1.0
            };
            (
                self.states[cf.from_clip].clip_index,
                self.states[cf.to_clip].clip_index,
                weight,
            )
        })
    }

    /// Evaluate the blend tree for the current state (if one exists).
    pub fn evaluate_blend_tree(&self) -> Option<Vec<(usize, f32)>> {
        self.blend_trees
            .get(&self.current_state)
            .map(|tree| tree.evaluate(&self.params_float))
    }

    /// Returns `true` if the state machine is currently crossfading.
    #[inline]
    pub fn is_transitioning(&self) -> bool {
        self.active_crossfade.is_some()
    }
}

impl Default for AnimationStateMachine {
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
    fn idle_to_walk_on_bool() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.add_transition(Transition {
            from: idle,
            to: walk,
            condition: Condition::BoolParam {
                name: "walking".into(),
                value: true,
            },
            blend_duration: 0.0,
            priority: 0,
        });

        assert_eq!(sm.current_state().name, "idle");

        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "idle");

        sm.set_bool("walking", true);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "walk");
    }

    #[test]
    fn walk_to_run_on_float() {
        let mut sm = AnimationStateMachine::new();
        let walk = sm.add_state(AnimState::new("walk", 0, 1.0));
        let run = sm.add_state(AnimState::new("run", 1, 1.0));

        sm.add_transition(Transition {
            from: walk,
            to: run,
            condition: Condition::FloatGreaterThan {
                name: "speed".into(),
                threshold: 5.0,
            },
            blend_duration: 0.0,
            priority: 0,
        });

        sm.set_float("speed", 3.0);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "walk");

        sm.set_float("speed", 6.0);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "run");
    }

    #[test]
    fn trigger_auto_resets() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let attack = sm.add_state(AnimState::new("attack", 1, 1.0));
        let idle2 = sm.add_state(AnimState::new("idle2", 2, 1.0));

        sm.add_transition(Transition {
            from: idle,
            to: attack,
            condition: Condition::Trigger {
                name: "attack".into(),
            },
            blend_duration: 0.0,
            priority: 0,
        });

        sm.add_transition(Transition {
            from: attack,
            to: idle2,
            condition: Condition::Trigger {
                name: "attack".into(),
            },
            blend_duration: 0.0,
            priority: 0,
        });

        sm.set_trigger("attack");
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "attack");

        // Trigger was consumed, so next evaluate should NOT transition
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "attack");

        // Set trigger again to transition from attack
        sm.set_trigger("attack");
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "idle2");
    }

    #[test]
    fn no_transition_when_condition_not_met() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let _walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.add_transition(Transition {
            from: idle,
            to: 1,
            condition: Condition::BoolParam {
                name: "walking".into(),
                value: true,
            },
            blend_duration: 0.0,
            priority: 0,
        });

        sm.set_bool("walking", false);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "idle");
    }

    #[test]
    fn crossfade_blend_weight() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.add_transition(Transition {
            from: idle,
            to: walk,
            condition: Condition::BoolParam {
                name: "walking".into(),
                value: true,
            },
            blend_duration: 0.5,
            priority: 0,
        });

        sm.set_bool("walking", true);
        sm.evaluate(0.0);

        // Crossfade should have started
        assert!(sm.active_crossfade.is_some());

        // Advance halfway through crossfade
        sm.evaluate(0.25);
        if let Some((from_clip, to_clip, weight)) = sm.blend_weight() {
            assert_eq!(from_clip, 0);
            assert_eq!(to_clip, 1);
            assert!(
                (weight - 0.5).abs() < 0.1,
                "expected weight ~0.5, got {weight}"
            );
        } else {
            panic!("expected active crossfade with blend weight");
        }

        // Complete crossfade
        sm.evaluate(0.3);
        assert!(sm.active_crossfade.is_none());
        assert_eq!(sm.current_state().name, "walk");
    }

    // -- New tests for extended functionality ----------------------------

    #[test]
    fn float_less_than_condition() {
        let mut sm = AnimationStateMachine::new();
        let run = sm.add_state(AnimState::new("run", 0, 1.0));
        let walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.add_transition(Transition::new(
            run,
            walk,
            Condition::FloatLessThan {
                name: "speed".into(),
                threshold: 3.0,
            },
            0.0,
        ));

        sm.set_float("speed", 5.0);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "run");

        sm.set_float("speed", 2.0);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "walk");
    }

    #[test]
    fn and_condition() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let _sprint = sm.add_state(AnimState::new("sprint", 1, 1.0));

        sm.add_transition(Transition::new(
            idle,
            1,
            Condition::And(vec![
                Condition::BoolParam {
                    name: "moving".into(),
                    value: true,
                },
                Condition::FloatGreaterThan {
                    name: "speed".into(),
                    threshold: 5.0,
                },
            ]),
            0.0,
        ));

        // Only moving=true, speed not set
        sm.set_bool("moving", true);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "idle");

        // Both conditions met
        sm.set_float("speed", 6.0);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "sprint");
    }

    #[test]
    fn or_condition() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let _jump = sm.add_state(AnimState::new("jump", 1, 1.0));

        sm.add_transition(Transition::new(
            idle,
            1,
            Condition::Or(vec![
                Condition::Trigger {
                    name: "jump".into(),
                },
                Condition::BoolParam {
                    name: "auto_jump".into(),
                    value: true,
                },
            ]),
            0.0,
        ));

        // auto_jump triggers the OR
        sm.set_bool("auto_jump", true);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "jump");
    }

    #[test]
    fn animation_finished_condition() {
        let mut sm = AnimationStateMachine::new();
        let attack = sm.add_state(AnimState::once("attack", 0, 1.0));
        let _idle = sm.add_state(AnimState::new("idle", 1, 1.0));

        sm.add_transition(Transition::new(
            attack,
            1,
            Condition::AnimationFinished,
            0.0,
        ));

        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "attack");

        sm.notify_animation_finished();
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "idle");
    }

    #[test]
    fn time_in_state_condition() {
        let mut sm = AnimationStateMachine::new();
        let wait = sm.add_state(AnimState::new("wait", 0, 1.0));
        let _go = sm.add_state(AnimState::new("go", 1, 1.0));

        sm.add_transition(Transition::new(wait, 1, Condition::TimeInState(1.0), 0.0));

        sm.evaluate(0.5);
        assert_eq!(sm.current_state().name, "wait");

        sm.evaluate(0.6); // total time_in_state > 1.0
        assert_eq!(sm.current_state().name, "go");
    }

    #[test]
    fn transition_priority() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let walk = sm.add_state(AnimState::new("walk", 1, 1.0));
        let run = sm.add_state(AnimState::new("run", 2, 1.0));

        // Both conditions will be true, but run has higher priority
        sm.add_transition(
            Transition::new(
                idle,
                walk,
                Condition::BoolParam {
                    name: "moving".into(),
                    value: true,
                },
                0.0,
            )
            .with_priority(0),
        );
        sm.add_transition(
            Transition::new(
                idle,
                run,
                Condition::BoolParam {
                    name: "moving".into(),
                    value: true,
                },
                0.0,
            )
            .with_priority(10),
        );

        sm.set_bool("moving", true);
        sm.evaluate(0.016);
        assert_eq!(sm.current_state().name, "run");
    }

    #[test]
    fn force_state() {
        let mut sm = AnimationStateMachine::new();
        let _idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let _walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.force_state(1);
        assert_eq!(sm.current_state().name, "walk");
        assert_eq!(sm.time_in_state, 0.0);
    }

    #[test]
    fn is_transitioning() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let _walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.add_transition(Transition::new(
            idle,
            1,
            Condition::BoolParam {
                name: "moving".into(),
                value: true,
            },
            0.5,
        ));

        assert!(!sm.is_transitioning());

        sm.set_bool("moving", true);
        sm.evaluate(0.0);
        assert!(sm.is_transitioning());
    }

    // -- Blend tree tests ------------------------------------------------

    #[test]
    fn blend_tree_single_clip() {
        let node = BlendNode::Clip {
            clip_index: 0,
            speed: 1.0,
        };
        let params = HashMap::new();
        let result = node.evaluate(&params);
        assert_eq!(result, vec![(0, 1.0)]);
    }

    #[test]
    fn blend_tree_1d_blend() {
        let node = BlendNode::Blend1D {
            param_name: "speed".into(),
            children: vec![
                (
                    0.0,
                    BlendNode::Clip {
                        clip_index: 0,
                        speed: 1.0,
                    },
                ),
                (
                    1.0,
                    BlendNode::Clip {
                        clip_index: 1,
                        speed: 1.0,
                    },
                ),
            ],
        };

        // At speed=0.0 -> fully clip 0
        let mut params = HashMap::new();
        params.insert("speed".into(), 0.0);
        let result = node.evaluate(&params);
        assert_eq!(result.len(), 2);
        assert!((result[0].1 - 1.0).abs() < 0.001); // clip 0 weight
        assert!((result[1].1 - 0.0).abs() < 0.001); // clip 1 weight

        // At speed=0.5 -> 50/50 blend
        params.insert("speed".into(), 0.5);
        let result = node.evaluate(&params);
        assert!((result[0].1 - 0.5).abs() < 0.001);
        assert!((result[1].1 - 0.5).abs() < 0.001);

        // At speed=1.0 -> fully clip 1
        params.insert("speed".into(), 1.0);
        let result = node.evaluate(&params);
        assert!((result[0].1 - 0.0).abs() < 0.001);
        assert!((result[1].1 - 1.0).abs() < 0.001);
    }

    #[test]
    fn blend_tree_additive() {
        let node = BlendNode::Additive {
            base: Box::new(BlendNode::Clip {
                clip_index: 0,
                speed: 1.0,
            }),
            additive: Box::new(BlendNode::Clip {
                clip_index: 1,
                speed: 1.0,
            }),
            weight_param: "additive_weight".into(),
        };

        let mut params = HashMap::new();
        params.insert("additive_weight".into(), 0.5);

        let result = node.evaluate(&params);
        assert_eq!(result.len(), 2);
        assert!((result[0].1 - 1.0).abs() < 0.001); // base at full weight
        assert!((result[1].1 - 0.5).abs() < 0.001); // additive at 0.5
    }

    #[test]
    fn state_machine_with_blend_tree() {
        let mut sm = AnimationStateMachine::new();
        let locomotion = sm.add_state(AnimState::new("locomotion", 0, 1.0));

        sm.set_blend_tree(
            locomotion,
            BlendNode::Blend1D {
                param_name: "speed".into(),
                children: vec![
                    (
                        0.0,
                        BlendNode::Clip {
                            clip_index: 0,
                            speed: 1.0,
                        },
                    ), // idle
                    (
                        1.0,
                        BlendNode::Clip {
                            clip_index: 1,
                            speed: 1.0,
                        },
                    ), // walk
                    (
                        2.0,
                        BlendNode::Clip {
                            clip_index: 2,
                            speed: 1.0,
                        },
                    ), // run
                ],
            },
        );

        sm.set_float("speed", 1.5);
        let weights = sm.evaluate_blend_tree();
        assert!(weights.is_some());
        let weights = weights.unwrap();
        // Should blend between walk (1) and run (2)
        assert!(weights.len() >= 2);
    }

    #[test]
    fn get_params() {
        let mut sm = AnimationStateMachine::new();
        sm.set_bool("grounded", true);
        sm.set_float("health", 100.0);

        assert_eq!(sm.get_bool("grounded"), Some(true));
        assert_eq!(sm.get_float("health"), Some(100.0));
        assert_eq!(sm.get_bool("missing"), None);
        assert_eq!(sm.get_float("missing"), None);
    }

    #[test]
    fn anim_state_once() {
        let state = AnimState::once("attack", 0, 1.5);
        assert!(!state.looping);
        assert_eq!(state.speed, 1.5);
    }

    #[test]
    fn time_in_state_resets_on_transition() {
        let mut sm = AnimationStateMachine::new();
        let idle = sm.add_state(AnimState::new("idle", 0, 1.0));
        let _walk = sm.add_state(AnimState::new("walk", 1, 1.0));

        sm.add_transition(Transition::new(
            idle,
            1,
            Condition::BoolParam {
                name: "moving".into(),
                value: true,
            },
            0.0,
        ));

        sm.evaluate(0.5); // time_in_state = 0.5
        assert!(sm.time_in_state > 0.4);

        sm.set_bool("moving", true);
        sm.evaluate(0.016);
        // After transition, time_in_state should reset
        assert!(sm.time_in_state < 0.1);
    }
}
