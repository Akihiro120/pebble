use std::{collections::HashMap, sync::Arc};

use super::{
    animation::AnimationClip,
    skeleton::{Skeleton, Transform},
};

/// A mutable snapshot of a skeleton's joint poses with world-space helpers.
///
/// Obtain one from [`AnimationPlayer::compute_pose`]. Optionally modify joints
/// for IK, then call [`skinning_matrices`](Self::skinning_matrices) to get
/// the final matrices ready for a [`SkinningBatch`](super::skinning::SkinnedBatchRenderer).
///
/// ```ignore
/// let mut pose = player.compute_pose();
/// let foot   = skeleton.joint_index_by_name("foot_l").unwrap();
/// let lower  = skeleton.joint_index_by_name("lower_leg_l").unwrap();
/// let upper  = skeleton.joint_index_by_name("upper_leg_l").unwrap();
///
/// // analytic two-bone IK — you supply the math, Pose handles local/world conversion
/// let (r_upper, r_lower) = two_bone_ik(
///     pose.world_position(upper),
///     pose.world_position(lower),
///     target_foot_pos,
/// );
/// pose.set_world_rotation(upper, r_upper);
/// pose.set_world_rotation(lower, r_lower);
///
/// let matrices = pose.skinning_matrices();
/// ```
pub struct Pose {
    locals:   Vec<Transform>,
    skeleton: Arc<Skeleton>,
    worlds:   Option<Vec<glam::Mat4>>, // lazily computed, invalidated on any set_*
}

impl Pose {
    pub(crate) fn new(locals: Vec<Transform>, skeleton: Arc<Skeleton>) -> Self {
        Self { locals, skeleton, worlds: None }
    }

    fn ensure_worlds(&mut self) {
        if self.worlds.is_none() {
            self.worlds = Some(self.skeleton.world_matrices(&self.locals));
        }
    }

    /// World-space position of `joint` (origin of its local frame).
    pub fn world_position(&mut self, joint: usize) -> glam::Vec3 {
        self.ensure_worlds();
        self.worlds.as_ref().unwrap()[joint].transform_point3(glam::Vec3::ZERO)
    }

    /// World-space rotation of `joint`.
    pub fn world_rotation(&mut self, joint: usize) -> glam::Quat {
        self.ensure_worlds();
        let (_, rot, _) = self.worlds.as_ref().unwrap()[joint].to_scale_rotation_translation();
        rot
    }

    /// Full world-space matrix of `joint` — position, rotation, and scale in
    /// one shot. Multiply by the entity's own world matrix to get game-world space.
    pub fn world_matrix(&mut self, joint: usize) -> glam::Mat4 {
        self.ensure_worlds();
        self.worlds.as_ref().unwrap()[joint]
    }

    /// Set `joint`'s orientation in world space. Converts to local
    /// (parent-relative) rotation automatically — no matrix math on your side.
    pub fn set_world_rotation(&mut self, joint: usize, rotation: glam::Quat) {
        let parent_rot = match self.skeleton.joint(joint).parent {
            Some(p) => {
                self.ensure_worlds();
                let (_, rot, _) = self.worlds.as_ref().unwrap()[p].to_scale_rotation_translation();
                rot
            }
            None => glam::Quat::IDENTITY,
        };
        self.locals[joint].rotation = parent_rot.inverse() * rotation;
        self.worlds = None;
    }

    /// Set `joint`'s position in world space. Converts to local
    /// (parent-relative) translation automatically.
    pub fn set_world_position(&mut self, joint: usize, position: glam::Vec3) {
        let parent_world = match self.skeleton.joint(joint).parent {
            Some(p) => {
                self.ensure_worlds();
                self.worlds.as_ref().unwrap()[p]
            }
            None => glam::Mat4::IDENTITY,
        };
        self.locals[joint].translation = parent_world.inverse().transform_point3(position);
        self.worlds = None;
    }

    /// Local (parent-relative) transform of `joint`.
    pub fn local(&self, joint: usize) -> Transform {
        self.locals[joint]
    }

    /// Directly set `joint`'s local (parent-relative) transform.
    /// Use when you already have the correct local-space value.
    pub fn set_local(&mut self, joint: usize, t: Transform) {
        self.locals[joint] = t;
        self.worlds = None;
    }

    /// Access the underlying skeleton — for name lookups, joint count, etc.
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// Compute skinning matrices from the current (possibly IK-modified) pose,
    /// ready to write into a [`SkinnedBatchRenderer`](super::skinning::SkinnedBatchRenderer).
    pub fn skinning_matrices(&self) -> Vec<glam::Mat4> {
        self.skeleton.skinning_matrices(&self.locals)
    }
}

struct Transition {
    from_clip: String,
    from_time: f32,
    elapsed: f32,
    duration: f32,
}

/// A component that manages animation playback state for a skinned entity.
///
/// Attach alongside a [`Handle<SkinnedMesh>`]. The engine advances time each
/// tick via [`advance`](Self::advance). Call [`play`](Self::play) /
/// [`crossfade`](Self::crossfade) to control playback. Obtain skinning
/// matrices via [`compute_matrices`](Self::compute_matrices) from your own
/// skinning system, or add [`CpuSkinningPlugin`](super::skinning::CpuSkinningPlugin)
/// to have the engine do it automatically.
///
/// Cloning shares the skeleton and clip data (via `Arc`) but resets
/// per-entity playback state — useful for spawning many entities from the
/// same loaded skeleton.
pub struct AnimationPlayer {
    skeleton: Arc<Skeleton>,
    clips: Arc<HashMap<String, AnimationClip>>,
    current: Option<String>,
    time: f32,
    speed: f32,
    looping: bool,
    transition: Option<Transition>,
    /// Set by [`set_matrices`](Self::set_matrices); cleared by [`clear_matrices`](Self::clear_matrices).
    /// When set, [`compute_matrices`](Self::compute_matrices) returns this instead of sampling the animation.
    matrices_override: Option<Vec<glam::Mat4>>,
}

impl AnimationPlayer {
    pub(crate) fn new(skeleton: Arc<Skeleton>, clips: Arc<HashMap<String, AnimationClip>>) -> Self {
        Self {
            skeleton,
            clips,
            current: None,
            time: 0.0,
            speed: 1.0,
            looping: true,
            transition: None,
            matrices_override: None,
        }
    }

    /// Switch immediately to `name`, restarting from the beginning. Use for
    /// hard cuts (hit reactions, death). Warns if `name` wasn't loaded.
    pub fn play(&mut self, name: &str) {
        if self.clips.contains_key(name) {
            self.current = Some(name.to_string());
            self.time = 0.0;
            self.looping = true;
            self.transition = None;
        } else {
            tracing::warn!("AnimationPlayer::play: clip '{name}' not found");
        }
    }

    /// Play `name` once to completion without looping, then hold the last
    /// frame. Warns if `name` wasn't loaded.
    pub fn play_once(&mut self, name: &str) {
        if self.clips.contains_key(name) {
            self.current = Some(name.to_string());
            self.time = 0.0;
            self.looping = false;
            self.transition = None;
        } else {
            tracing::warn!("AnimationPlayer::play_once: clip '{name}' not found");
        }
    }

    /// Blend from the current clip to `name` over `duration` seconds. Use
    /// for smooth transitions (walk → run). Warns if `name` wasn't loaded.
    pub fn crossfade(&mut self, name: &str, duration: f32) {
        if !self.clips.contains_key(name) {
            tracing::warn!("AnimationPlayer::crossfade: clip '{name}' not found");
            return;
        }
        if let Some(current) = &self.current {
            self.transition = Some(Transition {
                from_clip: current.clone(),
                from_time: self.time,
                elapsed: 0.0,
                duration: duration.max(f32::EPSILON),
            });
        }
        self.current = Some(name.to_string());
        self.time = 0.0;
        self.looping = true;
    }

    /// Stop advancing time (speed → 0).
    pub fn pause(&mut self) {
        self.speed = 0.0;
    }

    /// Resume normal playback (speed → 1).
    pub fn resume(&mut self) {
        self.speed = 1.0;
    }

    /// Set the playback speed multiplier. `1.0` = normal, `2.0` = double,
    /// negative values play in reverse.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// Jump to a specific time within the current clip.
    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    /// Current playback position in seconds.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Current playback speed multiplier.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Number of joints in this player's skeleton.
    pub fn joint_count(&self) -> usize {
        self.skeleton.joint_count()
    }

    /// The skeleton this player drives.
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// The currently active clip, if any.
    pub fn current_clip(&self) -> Option<&AnimationClip> {
        self.current.as_ref().and_then(|name| self.clips.get(name))
    }

    /// Returns an iterator over the names of all loaded animation clips, in an unspecified order.
    pub fn clip_names(&self) -> impl Iterator<Item = &str> {
        self.clips.keys().map(|s| s.as_str())
    }

    /// Advance time by `dt` seconds, handling looping and any active
    /// crossfade. Called by [`advance_animations`](super::skinning::advance_animations) —
    /// call it yourself only if you skip [`CpuSkinningPlugin`](super::skinning::CpuSkinningPlugin).
    pub fn advance(&mut self, dt: f32) {
        if let Some(t) = &mut self.transition {
            t.from_time += dt * self.speed;
            t.elapsed += dt;
            if t.elapsed >= t.duration {
                self.transition = None;
            }
        }
        let Some(name) = &self.current else { return };
        let Some(clip) = self.clips.get(name) else { return };
        self.time += dt * self.speed;
        if self.looping && clip.duration > 0.0 {
            self.time = self.time.rem_euclid(clip.duration);
        } else {
            self.time = self.time.min(clip.duration);
        }
    }

    /// Sample the current animation (including any active crossfade) into a
    /// [`Pose`]. Use as the IK entry point — modify joints via
    /// [`set_world_rotation`](Pose::set_world_rotation) /
    /// [`set_world_position`](Pose::set_world_position), then call
    /// [`skinning_matrices`](Pose::skinning_matrices). Returns the bind pose
    /// if no clip is playing.
    pub fn compute_pose(&self) -> Pose {
        let locals = self.sample_locals();
        Pose::new(locals, Arc::clone(&self.skeleton))
    }

    fn sample_locals(&self) -> Vec<Transform> {
        let Some(name) = &self.current else {
            return self.skeleton.bind_pose();
        };
        let Some(clip) = self.clips.get(name) else {
            return self.skeleton.bind_pose();
        };
        match &self.transition {
            None => clip.sample(self.time, &self.skeleton),
            Some(t) => {
                let weight = (t.elapsed / t.duration).clamp(0.0, 1.0);
                let poses_to = clip.sample(self.time, &self.skeleton);
                if let Some(from) = self.clips.get(&t.from_clip) {
                    let poses_from = from.sample(t.from_time, &self.skeleton);
                    poses_from.iter().zip(&poses_to).map(|(a, b)| a.lerp(b, weight)).collect()
                } else {
                    poses_to
                }
            }
        }
    }

    /// Override the matrices that [`compute_matrices`](Self::compute_matrices)
    /// returns. Use after modifying a [`Pose`] for IK or procedural animation:
    ///
    /// ```ignore
    /// let mut pose = player.compute_pose();
    /// pose.set_world_rotation(foot, target_rot);
    /// player.set_matrices(pose.skinning_matrices());
    /// ```
    ///
    /// The override persists until [`clear_matrices`](Self::clear_matrices) is
    /// called, so call this every frame while the override is active.
    pub fn set_matrices(&mut self, matrices: Vec<glam::Mat4>) {
        self.matrices_override = Some(matrices);
    }

    /// Remove the matrix override — [`compute_matrices`](Self::compute_matrices)
    /// returns to sampling the current animation clip.
    pub fn clear_matrices(&mut self) {
        self.matrices_override = None;
    }

    /// Sample the current animation (including any active crossfade) and
    /// return one skinning matrix per joint, ready to upload. Returns the
    /// matrix override if one is set via [`set_matrices`](Self::set_matrices),
    /// or identity matrices if no clip is playing.
    pub fn compute_matrices(&self) -> Vec<glam::Mat4> {
        if let Some(ref m) = self.matrices_override {
            return m.clone();
        }
        if self.current.is_none() {
            return vec![glam::Mat4::IDENTITY; self.skeleton.joint_count()];
        }
        self.compute_pose().skinning_matrices()
    }
}

impl Clone for AnimationPlayer {
    fn clone(&self) -> Self {
        Self {
            skeleton: Arc::clone(&self.skeleton),
            clips: Arc::clone(&self.clips),
            current: self.current.clone(),
            time: 0.0,
            speed: 1.0,
            looping: true,
            transition: None,
            matrices_override: None,
        }
    }
}
