//! Plain CPU data for keyframe animation — no [`Asset`](crate::assets::upload::Asset)/
//! `Handle`/GPU upload involved, same rationale as [`skeleton`](super::skeleton):
//! sampling a clip is pure interpolation math, nothing GPU-specific about it.

use super::skeleton::{Skeleton, Transform};

/// glTF 2.0 sampler interpolation. `CubicSpline` is intentionally absent as
/// a variant — [`gltf_loader::load_gltf`](super::gltf_loader::load_gltf)
/// hard-errors on it rather than silently mishandling its 3-value-per-keyframe
/// (in-tangent/value/out-tangent) packing, which would otherwise be
/// misread as plain values.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
}

/// One sample of an animated property at a point in time.
#[derive(Copy, Clone, Debug)]
pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
}

/// One joint's animated properties. Any of the three tracks may be empty —
/// glTF allows animating only some of a joint's translation/rotation/scale;
/// [`AnimationClip::sample`] falls back to the joint's bind pose for
/// whichever are unanimated. Keyframes within each track must be in
/// non-decreasing `time` order (glTF's own accessors are spec-guaranteed to
/// already be).
pub struct JointTrack {
    /// Index into the [`Skeleton`] this track animates.
    pub joint_index: usize,
    pub translation: Vec<Keyframe<glam::Vec3>>,
    pub translation_interpolation: Interpolation,
    pub rotation: Vec<Keyframe<glam::Quat>>,
    pub rotation_interpolation: Interpolation,
    pub scale: Vec<Keyframe<glam::Vec3>>,
    pub scale_interpolation: Interpolation,
}

/// A single animation — a name, a duration, and per-joint keyframe tracks.
/// Blending multiple clips (crossfade, additive layering, ...) is entirely
/// up to you: sample each clip separately and combine the resulting
/// `Transform`s yourself — this type only ever samples one clip at a time.
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    tracks: Vec<JointTrack>,
}

impl AnimationClip {
    /// `duration` is computed as the latest keyframe time across every
    /// track/property — not passed in, so it can never drift out of sync
    /// with the actual keyframe data.
    pub fn new(name: String, tracks: Vec<JointTrack>) -> Self {
        let duration = tracks
            .iter()
            .flat_map(|t| {
                t.translation
                    .iter()
                    .map(|k| k.time)
                    .chain(t.rotation.iter().map(|k| k.time))
                    .chain(t.scale.iter().map(|k| k.time))
            })
            .fold(0.0f32, f32::max);
        Self { name, duration, tracks }
    }

    /// Samples every joint's local pose at `time`, **clamped** to
    /// `[0, duration]` — not wrapped. To loop, pass
    /// `time.rem_euclid(clip.duration)` yourself before calling. `skeleton`
    /// supplies the bind-pose fallback for any joint this clip has no track
    /// for, and for whichever of translation/rotation/scale a joint's track
    /// leaves unspecified. Output is indexed identically to `skeleton` —
    /// feed straight into [`Skeleton::world_matrices`]/[`skinning_matrices`](Skeleton::skinning_matrices).
    pub fn sample(&self, time: f32, skeleton: &Skeleton) -> Vec<Transform> {
        let time = time.clamp(0.0, self.duration);
        (0..skeleton.joint_count())
            .map(|i| {
                let bind = skeleton.joint(i).local_bind_transform;
                match self.tracks.iter().find(|t| t.joint_index == i) {
                    Some(track) => Transform {
                        translation: sample_vec3(&track.translation, track.translation_interpolation, time)
                            .unwrap_or(bind.translation),
                        rotation: sample_quat(&track.rotation, track.rotation_interpolation, time)
                            .unwrap_or(bind.rotation),
                        scale: sample_vec3(&track.scale, track.scale_interpolation, time).unwrap_or(bind.scale),
                    },
                    None => bind,
                }
            })
            .collect()
    }
}

/// Where `time` falls relative to a (non-decreasing, by construction)
/// keyframe track.
enum Bracket<'a, T> {
    /// No keyframes at all — the property isn't animated.
    Empty,
    /// `time` is at or beyond one end of the track (or the track has only
    /// one keyframe) — clamp to that single value.
    Single(&'a T),
    /// `time` falls strictly between two keyframes; `t` is the normalized
    /// (0..=1) position between them.
    Between { left: &'a T, right: &'a T, t: f32 },
}

fn bracket<T>(keyframes: &[Keyframe<T>], time: f32) -> Bracket<'_, T> {
    if keyframes.is_empty() {
        return Bracket::Empty;
    }
    if keyframes.len() == 1 || time <= keyframes[0].time {
        return Bracket::Single(&keyframes[0].value);
    }
    let last = keyframes.len() - 1;
    if time >= keyframes[last].time {
        return Bracket::Single(&keyframes[last].value);
    }

    // First index whose time is > `time` is always >= 1 here (time is
    // already known to be > keyframes[0].time from the checks above).
    let right_index = keyframes.partition_point(|k| k.time <= time);
    let left = &keyframes[right_index - 1];
    let right = &keyframes[right_index];
    let span = right.time - left.time;
    let t = if span > 0.0 { (time - left.time) / span } else { 0.0 };
    Bracket::Between { left: &left.value, right: &right.value, t }
}

fn sample_vec3(keyframes: &[Keyframe<glam::Vec3>], interpolation: Interpolation, time: f32) -> Option<glam::Vec3> {
    Some(match bracket(keyframes, time) {
        Bracket::Empty => return None,
        Bracket::Single(v) => *v,
        Bracket::Between { left, right, t } => match interpolation {
            Interpolation::Step => *left,
            Interpolation::Linear => left.lerp(*right, t),
        },
    })
}

fn sample_quat(keyframes: &[Keyframe<glam::Quat>], interpolation: Interpolation, time: f32) -> Option<glam::Quat> {
    Some(match bracket(keyframes, time) {
        Bracket::Empty => return None,
        Bracket::Single(v) => *v,
        Bracket::Between { left, right, t } => match interpolation {
            Interpolation::Step => *left,
            Interpolation::Linear => left.slerp(*right, t),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::skeleton::Joint;

    fn skeleton_with_one_joint() -> Skeleton {
        Skeleton::new(vec![Joint {
            name: "root".to_string(),
            parent: None,
            inverse_bind_matrix: glam::Mat4::IDENTITY,
            local_bind_transform: Transform {
                translation: glam::Vec3::new(9.0, 9.0, 9.0),
                ..Transform::IDENTITY
            },
        }])
    }

    fn track_with_translation(keyframes: Vec<Keyframe<glam::Vec3>>, interpolation: Interpolation) -> JointTrack {
        JointTrack {
            joint_index: 0,
            translation: keyframes,
            translation_interpolation: interpolation,
            rotation: Vec::new(),
            rotation_interpolation: Interpolation::Linear,
            scale: Vec::new(),
            scale_interpolation: Interpolation::Linear,
        }
    }

    #[test]
    fn time_before_the_first_keyframe_clamps_to_it() {
        let skeleton = skeleton_with_one_joint();
        let track = track_with_translation(
            vec![
                Keyframe { time: 1.0, value: glam::Vec3::new(1.0, 0.0, 0.0) },
                Keyframe { time: 2.0, value: glam::Vec3::new(2.0, 0.0, 0.0) },
            ],
            Interpolation::Linear,
        );
        let clip = AnimationClip::new("clip".to_string(), vec![track]);

        let poses = clip.sample(-5.0, &skeleton);
        assert_eq!(poses[0].translation, glam::Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn time_after_the_last_keyframe_clamps_to_it() {
        let skeleton = skeleton_with_one_joint();
        let track = track_with_translation(
            vec![
                Keyframe { time: 1.0, value: glam::Vec3::new(1.0, 0.0, 0.0) },
                Keyframe { time: 2.0, value: glam::Vec3::new(2.0, 0.0, 0.0) },
            ],
            Interpolation::Linear,
        );
        let clip = AnimationClip::new("clip".to_string(), vec![track]);

        let poses = clip.sample(100.0, &skeleton);
        assert_eq!(poses[0].translation, glam::Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn exactly_on_a_keyframe_returns_the_exact_value_no_drift() {
        let skeleton = skeleton_with_one_joint();
        let track = track_with_translation(
            vec![
                Keyframe { time: 0.0, value: glam::Vec3::new(1.0, 0.0, 0.0) },
                Keyframe { time: 1.0, value: glam::Vec3::new(2.0, 0.0, 0.0) },
                Keyframe { time: 2.0, value: glam::Vec3::new(3.0, 0.0, 0.0) },
            ],
            Interpolation::Linear,
        );
        let clip = AnimationClip::new("clip".to_string(), vec![track]);

        let poses = clip.sample(1.0, &skeleton);
        assert_eq!(poses[0].translation, glam::Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn midpoint_lerps_linearly() {
        let skeleton = skeleton_with_one_joint();
        let track = track_with_translation(
            vec![
                Keyframe { time: 0.0, value: glam::Vec3::new(0.0, 0.0, 0.0) },
                Keyframe { time: 2.0, value: glam::Vec3::new(10.0, 0.0, 0.0) },
            ],
            Interpolation::Linear,
        );
        let clip = AnimationClip::new("clip".to_string(), vec![track]);

        let poses = clip.sample(1.0, &skeleton);
        assert_eq!(poses[0].translation, glam::Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn step_interpolation_holds_the_left_keyframe() {
        let skeleton = skeleton_with_one_joint();
        let track = track_with_translation(
            vec![
                Keyframe { time: 0.0, value: glam::Vec3::new(0.0, 0.0, 0.0) },
                Keyframe { time: 2.0, value: glam::Vec3::new(10.0, 0.0, 0.0) },
            ],
            Interpolation::Step,
        );
        let clip = AnimationClip::new("clip".to_string(), vec![track]);

        let poses = clip.sample(1.9, &skeleton);
        assert_eq!(poses[0].translation, glam::Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn a_partially_animated_joint_falls_back_to_bind_pose_for_untracked_properties() {
        let skeleton = skeleton_with_one_joint();
        // Only translation is tracked — rotation/scale must fall back to bind pose.
        let track = track_with_translation(
            vec![Keyframe { time: 0.0, value: glam::Vec3::new(1.0, 2.0, 3.0) }],
            Interpolation::Linear,
        );
        let clip = AnimationClip::new("clip".to_string(), vec![track]);

        let poses = clip.sample(0.0, &skeleton);
        assert_eq!(poses[0].translation, glam::Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(poses[0].rotation, glam::Quat::IDENTITY);
        assert_eq!(poses[0].scale, glam::Vec3::ONE);
    }

    #[test]
    fn a_joint_with_no_track_at_all_is_fully_bind_pose() {
        let skeleton = skeleton_with_one_joint();
        let clip = AnimationClip::new("clip".to_string(), vec![]);

        let poses = clip.sample(0.0, &skeleton);
        assert_eq!(poses[0], Transform { translation: glam::Vec3::new(9.0, 9.0, 9.0), ..Transform::IDENTITY });
    }

    #[test]
    fn duration_is_the_max_keyframe_time_across_every_track_and_property() {
        let track = JointTrack {
            joint_index: 0,
            translation: vec![Keyframe { time: 1.0, value: glam::Vec3::ZERO }],
            translation_interpolation: Interpolation::Linear,
            rotation: vec![Keyframe { time: 5.0, value: glam::Quat::IDENTITY }],
            rotation_interpolation: Interpolation::Linear,
            scale: vec![Keyframe { time: 3.0, value: glam::Vec3::ONE }],
            scale_interpolation: Interpolation::Linear,
        };
        let clip = AnimationClip::new("clip".to_string(), vec![track]);
        assert_eq!(clip.duration, 5.0);
    }
}
