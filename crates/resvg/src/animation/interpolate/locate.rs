// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use usvg::{CalcMode, Easing, Keyframe, PathKeyframe, TimingFunction};

use super::super::easing::{apply_timing_function, key_spline};

/// Provides the locator's common offset and timing access for track keyframes.
pub(super) trait TrackKeyframe {
    /// The keyframe offset in [0, 1].
    fn keyframe_offset(&self) -> f32;

    /// The per-keyframe timing function, if any.
    fn keyframe_timing_function(&self) -> Option<TimingFunction>;
}

impl<T: Clone> TrackKeyframe for Keyframe<T> {
    fn keyframe_offset(&self) -> f32 {
        self.offset().get()
    }

    fn keyframe_timing_function(&self) -> Option<TimingFunction> {
        self.timing_function().copied()
    }
}

impl TrackKeyframe for PathKeyframe {
    fn keyframe_offset(&self) -> f32 {
        self.offset().get()
    }

    fn keyframe_timing_function(&self) -> Option<TimingFunction> {
        self.timing_function().copied()
    }
}

/// Locates the sampling position within a typed keyframe track.
///
/// Returns the bracketing `(low, high)` keyframe indices and the eased segment
/// parameter `t`, or `None` when the track is empty. `paced_distances`, when
/// present, drives arc-length spacing under `calcMode="paced"`.
pub(super) fn locate_track<T: Clone>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
    paced_distances: Option<Vec<f32>>,
) -> Option<(usize, usize, f32)> {
    if keyframes.is_empty() {
        return None;
    }

    Some(locate(
        keyframes,
        easing,
        timing_function,
        progress,
        paced_distances.as_deref(),
    ))
}

/// Locates the sampling position from keyframes.
pub(super) fn locate<T: TrackKeyframe>(
    keyframes: &[T],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
    paced_distances: Option<&[f32]>,
) -> (usize, usize, f32) {
    if keyframes.len() <= 1 {
        return (0, 0, 0.0);
    }

    let progress = progress.clamp(0.0, 1.0);
    match easing.calc_mode() {
        CalcMode::Discrete => {
            let index = discrete_index(keyframes, progress);
            (index, index, 0.0)
        }
        CalcMode::Paced => match paced_distances {
            Some(distances) => paced_bracket(distances, progress),
            None => {
                warn_paced_unsupported();
                bracket(keyframes, progress)
            }
        },
        CalcMode::Linear | CalcMode::Spline => {
            let (lo, hi, local) = bracket(keyframes, progress);
            let eased = ease_segment(easing, keyframes, timing_function, lo, local);
            (lo, hi, eased)
        }
    }
}

/// Brackets `progress` against keyframe offsets, returning the raw local ratio.
fn bracket<T: TrackKeyframe>(keyframes: &[T], progress: f32) -> (usize, usize, f32) {
    let count = keyframes.len();
    for i in 0..count - 1 {
        let end = keyframes[i + 1].keyframe_offset();
        if progress < end {
            let start = keyframes[i].keyframe_offset();
            let span = end - start;
            let local = if span > 0.0 {
                ((progress - start) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return (i, i + 1, local);
        }
    }
    // At or past the last offset the final value is held.
    (count - 1, count - 1, 0.0)
}

/// Brackets `progress` against raw offsets, returning the raw local ratio.
pub(super) fn bracket_offsets(offsets: &[f32], progress: f32) -> (usize, usize, f32) {
    let count = offsets.len();
    for i in 0..count - 1 {
        let end = offsets[i + 1];
        if progress < end {
            let start = offsets[i];
            let span = end - start;
            let local = if span > 0.0 {
                ((progress - start) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return (i, i + 1, local);
        }
    }
    // At or past the last offset the final value is held.
    (count - 1, count - 1, 0.0)
}

/// Brackets `progress` by cumulative per-segment distance for `paced` mode.
fn paced_bracket(distances: &[f32], progress: f32) -> (usize, usize, f32) {
    let total: f32 = distances.iter().sum();
    if total <= 0.0 {
        return (0, 0, 0.0);
    }

    let target = progress * total;
    let last = distances.len() - 1;
    let mut traveled = 0.0;
    for (i, &segment) in distances.iter().enumerate() {
        if target < traveled + segment || i == last {
            let local = if segment > 0.0 {
                ((target - traveled) / segment).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return (i, i + 1, local);
        }
        traveled += segment;
    }
    (last, last + 1, 1.0)
}

/// Returns the index whose discrete value is active at `progress`.
pub(super) fn discrete_index<T: TrackKeyframe>(keyframes: &[T], progress: f32) -> usize {
    let mut index = 0;
    for (i, keyframe) in keyframes.iter().enumerate() {
        if keyframe.keyframe_offset() <= progress {
            index = i;
        } else {
            break;
        }
    }
    index
}

/// Shapes a raw segment ratio by the spline or CSS per-keyframe easing.
fn ease_segment<T: TrackKeyframe>(
    easing: &Easing,
    keyframes: &[T],
    timing_function: Option<&TimingFunction>,
    segment: usize,
    local: f32,
) -> f32 {
    match easing.calc_mode() {
        CalcMode::Spline => easing
            .key_splines()
            .and_then(|splines| splines.get(segment))
            .map(|spline| key_spline(*spline, local))
            .unwrap_or(local),
        _ => keyframes
            .get(segment)
            .and_then(|keyframe| keyframe.keyframe_timing_function())
            .or(timing_function.copied())
            .map(|tf| apply_timing_function(&tf, local))
            .unwrap_or(local),
    }
}

/// Computes per-segment paced distances when `calcMode="paced"`, else `None`.
pub(super) fn paced_of<T: Clone>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    metric: impl Fn(&T, &T) -> f32,
) -> Option<Vec<f32>> {
    matches!(easing.calc_mode(), CalcMode::Paced).then(|| segment_metrics(keyframes, metric))
}

/// Maps each adjacent keyframe pair to its distance under `metric`.
pub(super) fn segment_metrics<T: Clone>(
    keyframes: &[Keyframe<T>],
    metric: impl Fn(&T, &T) -> f32,
) -> Vec<f32> {
    (0..keyframes.len().saturating_sub(1))
        .map(|i| metric(keyframes[i].value(), keyframes[i + 1].value()))
        .collect()
}

/// Linearly interpolates between `a` and `b`.
pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn warn_paced_unsupported() {
    log::warn!("Paced interpolation is not supported here; using linear.");
}
