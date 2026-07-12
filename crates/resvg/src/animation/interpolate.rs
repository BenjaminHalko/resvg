// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-kind value interpolation for animation tracks.
//!
//! [`interpolate_track`] samples one [`usvg::AnimationKind`] at a normalized
//! progress within its simple duration and returns the typed value. Scalars and
//! opacities lerp linearly, colors lerp per sRGB channel, SMIL transform
//! parameters lerp before the matrix is built, CSS transform function lists lerp
//! only when structurally compatible (otherwise they step discretely), baked
//! path tracks lerp point-wise, and `animateMotion` maps progress onto the path
//! by arc length.
//!
//! The `calcMode` in [`usvg::Easing`] selects the segment behavior: `linear` and
//! `spline` interpolate between the two bracketing keyframes (splines and CSS
//! per-keyframe timing functions shape the segment parameter), `discrete` steps,
//! and `paced` spaces the keyframes by a per-kind distance metric.

use std::sync::Arc;

use svgtypes::Color;
use tiny_skia::{Path, PathBuilder, PathSegment, Point, Transform};
use usvg::{
    AnimationKind, AnimationVisibility, CalcMode, Easing, FillRule, Keyframe, LineCap, LineJoin,
    MotionRotate, MotionTrack, NonZeroRect, NormalizedF32, PathTrack, StrokeMiterlimit,
    TimingFunction, TransformFunction, TransformKind, TransformTrack,
};

use super::easing::{apply_timing_function, key_spline};

/// A single sampled animation value, typed by the track it came from.
///
/// The `transform-origin` of a CSS transform is intentionally not applied here:
/// resolving a percentage or box-relative origin needs the node bounding box,
/// which the composition layer supplies. Stop offsets are reported as
/// [`SampledValue::GradientGeometry`]; the caller knows the originating kind.
#[derive(Clone, Debug)]
pub(crate) enum SampledValue {
    /// A `transform` or `gradientTransform` matrix.
    Transform(Transform),
    /// An `opacity` or `stop-opacity` value.
    Opacity(f32),
    /// A `fill`, `stroke`, or `stop-color` value.
    Color(Color),
    /// A `stroke-width` value.
    StrokeWidth(f32),
    /// A `stroke-dashoffset` value.
    StrokeDashoffset(f32),
    /// A `stroke-dasharray` value.
    StrokeDasharray(Vec<f32>),
    /// A `stroke-miterlimit` value.
    StrokeMiterlimit(f32),
    /// A `stroke-linecap` value.
    StrokeLinecap(LineCap),
    /// A `stroke-linejoin` value.
    StrokeLinejoin(LineJoin),
    /// A `fill-rule` value.
    FillRule(FillRule),
    /// A `display` value (`true` shows the element).
    Display(bool),
    /// A `visibility` value.
    Visibility(AnimationVisibility),
    /// A baked geometry snapshot and whether it renders anything.
    Path(Arc<Path>, bool),
    /// A gradient geometry scalar or stop offset.
    GradientGeometry(f32),
    /// A `viewBox` rect.
    ViewBox(NonZeroRect),
    /// An `image` geometry scalar (`x`, `y`, `width`, or `height`).
    ImageGeometry(f32),
    /// The local transform contributed by `animateMotion`.
    Motion(Transform),
}

/// Interpolates a track at `progress` (`0.0..=1.0`) within its simple duration.
///
/// Returns `None` only when the track carries no keyframes (which the parser
/// never produces) or when a sampled value cannot form a valid shape.
pub(crate) fn interpolate_track(
    kind: &AnimationKind,
    easing: &Easing,
    progress: f32,
) -> Option<SampledValue> {
    match kind {
        AnimationKind::Transform(track) | AnimationKind::GradientTransform(track) => {
            sample_transform(track, easing, progress).map(SampledValue::Transform)
        }
        AnimationKind::Motion(track) => {
            sample_motion(track, easing, progress).map(SampledValue::Motion)
        }
        AnimationKind::Opacity(track) | AnimationKind::StopOpacity(track) => {
            sample_opacity(track.keyframes(), easing, progress).map(SampledValue::Opacity)
        }
        AnimationKind::Fill(track)
        | AnimationKind::Stroke(track)
        | AnimationKind::StopColor(track) => {
            sample_color(track.keyframes(), easing, progress).map(SampledValue::Color)
        }
        AnimationKind::StrokeWidth(track) => {
            sample_scalar(track.keyframes(), easing, progress).map(SampledValue::StrokeWidth)
        }
        AnimationKind::StrokeDashoffset(track) => {
            sample_scalar(track.keyframes(), easing, progress).map(SampledValue::StrokeDashoffset)
        }
        AnimationKind::StrokeDasharray(track) => {
            sample_dasharray(track.keyframes(), easing, progress).map(SampledValue::StrokeDasharray)
        }
        AnimationKind::StrokeMiterlimit(track) => {
            sample_miterlimit(track.keyframes(), easing, progress)
                .map(SampledValue::StrokeMiterlimit)
        }
        AnimationKind::StrokeLinecap(track) => {
            sample_discrete(track.keyframes(), easing, progress).map(SampledValue::StrokeLinecap)
        }
        AnimationKind::StrokeLinejoin(track) => {
            sample_discrete(track.keyframes(), easing, progress).map(SampledValue::StrokeLinejoin)
        }
        AnimationKind::FillRule(track) => {
            sample_discrete(track.keyframes(), easing, progress).map(SampledValue::FillRule)
        }
        AnimationKind::Display(track) => {
            sample_discrete(track.keyframes(), easing, progress).map(SampledValue::Display)
        }
        AnimationKind::Visibility(track) => {
            sample_discrete(track.keyframes(), easing, progress).map(SampledValue::Visibility)
        }
        AnimationKind::Path(track) => sample_path(track, easing, progress)
            .map(|(path, renderable)| SampledValue::Path(path, renderable)),
        AnimationKind::StopOffset(track) => {
            sample_opacity(track.keyframes(), easing, progress).map(SampledValue::GradientGeometry)
        }
        AnimationKind::GradientGeometry(track) => {
            sample_scalar(track.keyframes(), easing, progress).map(SampledValue::GradientGeometry)
        }
        AnimationKind::ViewBox(track) => {
            sample_viewbox(track.keyframes(), easing, progress).map(SampledValue::ViewBox)
        }
        AnimationKind::ImageX(track)
        | AnimationKind::ImageY(track)
        | AnimationKind::ImageWidth(track)
        | AnimationKind::ImageHeight(track) => {
            sample_scalar(track.keyframes(), easing, progress).map(SampledValue::ImageGeometry)
        }
    }
}

// --- Keyframe location ------------------------------------------------------

/// Locates the sampling position within a typed keyframe track.
///
/// Returns the bracketing `(low, high)` keyframe indices and the eased segment
/// parameter `t`, or `None` when the track is empty. `paced_distances`, when
/// present, drives arc-length spacing under `calcMode="paced"`.
fn locate_track<T: Clone>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    progress: f32,
    paced_distances: Option<Vec<f32>>,
) -> Option<(usize, usize, f32)> {
    if keyframes.is_empty() {
        return None;
    }

    let offsets: Vec<f32> = keyframes.iter().map(|k| k.offset().get()).collect();
    let timings: Vec<Option<TimingFunction>> =
        keyframes.iter().map(|k| k.timing_function().copied()).collect();

    Some(locate(
        &offsets,
        &timings,
        easing,
        progress,
        paced_distances.as_deref(),
    ))
}

/// Locates the sampling position from raw offset and timing slices.
fn locate(
    offsets: &[f32],
    timings: &[Option<TimingFunction>],
    easing: &Easing,
    progress: f32,
    paced_distances: Option<&[f32]>,
) -> (usize, usize, f32) {
    if offsets.len() <= 1 {
        return (0, 0, 0.0);
    }

    let progress = progress.clamp(0.0, 1.0);
    match easing.calc_mode() {
        CalcMode::Discrete => {
            let index = discrete_index(offsets, progress);
            (index, index, 0.0)
        }
        CalcMode::Paced => match paced_distances {
            Some(distances) => paced_bracket(distances, progress),
            None => {
                warn_paced_unsupported();
                bracket(offsets, progress)
            }
        },
        CalcMode::Linear | CalcMode::Spline => {
            let (lo, hi, local) = bracket(offsets, progress);
            let eased = ease_segment(easing, timings, lo, local);
            (lo, hi, eased)
        }
    }
}

/// Brackets `progress` against keyframe offsets, returning the raw local ratio.
fn bracket(offsets: &[f32], progress: f32) -> (usize, usize, f32) {
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
fn discrete_index(offsets: &[f32], progress: f32) -> usize {
    let mut index = 0;
    for (i, &offset) in offsets.iter().enumerate() {
        if offset <= progress {
            index = i;
        } else {
            break;
        }
    }
    index
}

/// Shapes a raw segment ratio by the spline or CSS per-keyframe easing.
fn ease_segment(
    easing: &Easing,
    timings: &[Option<TimingFunction>],
    segment: usize,
    local: f32,
) -> f32 {
    match easing.calc_mode() {
        CalcMode::Spline => easing
            .key_splines()
            .and_then(|splines| splines.get(segment))
            .map(|spline| key_spline(*spline, local))
            .unwrap_or(local),
        _ => timings
            .get(segment)
            .copied()
            .flatten()
            .map(|tf| apply_timing_function(&tf, local))
            .unwrap_or(local),
    }
}

/// Computes per-segment paced distances when `calcMode="paced"`, else `None`.
fn paced_of<T: Clone>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    metric: impl Fn(&T, &T) -> f32,
) -> Option<Vec<f32>> {
    matches!(easing.calc_mode(), CalcMode::Paced).then(|| segment_metrics(keyframes, metric))
}

/// Maps each adjacent keyframe pair to its distance under `metric`.
fn segment_metrics<T: Clone>(keyframes: &[Keyframe<T>], metric: impl Fn(&T, &T) -> f32) -> Vec<f32> {
    (0..keyframes.len().saturating_sub(1))
        .map(|i| metric(keyframes[i].value(), keyframes[i + 1].value()))
        .collect()
}

/// Linearly interpolates between `a` and `b`.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// --- Scalar and simple value tracks -----------------------------------------

/// Samples a plain `f32` track (stroke width, geometry, image geometry).
fn sample_scalar(keyframes: &[Keyframe<f32>], easing: &Easing, progress: f32) -> Option<f32> {
    let paced = paced_of(keyframes, easing, |a, b| (a - b).abs());
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    Some(lerp(*keyframes[lo].value(), *keyframes[hi].value(), t))
}

/// Samples a normalized track (`opacity`, `stop-opacity`, `stop` offset).
fn sample_opacity(
    keyframes: &[Keyframe<NormalizedF32>],
    easing: &Easing,
    progress: f32,
) -> Option<f32> {
    let paced = paced_of(keyframes, easing, |a, b| (a.get() - b.get()).abs());
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    let value = lerp(keyframes[lo].value().get(), keyframes[hi].value().get(), t);
    Some(value.clamp(0.0, 1.0))
}

/// Samples a `stroke-miterlimit` track.
fn sample_miterlimit(
    keyframes: &[Keyframe<StrokeMiterlimit>],
    easing: &Easing,
    progress: f32,
) -> Option<f32> {
    let paced = paced_of(keyframes, easing, |a, b| (a.get() - b.get()).abs());
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    Some(lerp(keyframes[lo].value().get(), keyframes[hi].value().get(), t))
}

/// Samples a color track by lerping each sRGB channel.
fn sample_color(keyframes: &[Keyframe<Color>], easing: &Easing, progress: f32) -> Option<Color> {
    let paced = paced_of(keyframes, easing, color_distance);
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    Some(lerp_color(keyframes[lo].value(), keyframes[hi].value(), t))
}

/// Lerps two colors channel-wise in sRGB space.
fn lerp_color(a: &Color, b: &Color, t: f32) -> Color {
    Color::new_rgba(
        lerp_channel(a.red, b.red, t),
        lerp_channel(a.green, b.green, t),
        lerp_channel(a.blue, b.blue, t),
        lerp_channel(a.alpha, b.alpha, t),
    )
}

/// Lerps a single 8-bit color channel, rounding to the nearest value.
fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    lerp(f32::from(a), f32::from(b), t).round().clamp(0.0, 255.0) as u8
}

/// The Euclidean distance between two colors over RGBA channels.
fn color_distance(a: &Color, b: &Color) -> f32 {
    let dr = f32::from(a.red) - f32::from(b.red);
    let dg = f32::from(a.green) - f32::from(b.green);
    let db = f32::from(a.blue) - f32::from(b.blue);
    let da = f32::from(a.alpha) - f32::from(b.alpha);
    (dr * dr + dg * dg + db * db + da * da).sqrt()
}

/// Samples a `stroke-dasharray` track element-wise.
fn sample_dasharray(
    keyframes: &[Keyframe<Vec<f32>>],
    easing: &Easing,
    progress: f32,
) -> Option<Vec<f32>> {
    let paced = paced_of(keyframes, easing, |a, b| {
        let len = a.len().min(b.len());
        (0..len).map(|i| (a[i] - b[i]).abs()).sum()
    });
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    let a = keyframes[lo].value();
    let b = keyframes[hi].value();
    let len = a.len().min(b.len());
    Some((0..len).map(|i| lerp(a[i], b[i], t)).collect())
}

/// Samples a discrete-only track (enums, `display`) by holding the low keyframe.
fn sample_discrete<T: Copy>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    progress: f32,
) -> Option<T> {
    let (lo, _, _) = locate_track(keyframes, easing, progress, None)?;
    Some(*keyframes[lo].value())
}

/// Samples a `viewBox` track by lerping each rect component.
fn sample_viewbox(
    keyframes: &[Keyframe<NonZeroRect>],
    easing: &Easing,
    progress: f32,
) -> Option<NonZeroRect> {
    let paced = paced_of(keyframes, easing, |a, b| {
        let dx = a.x() - b.x();
        let dy = a.y() - b.y();
        let dw = a.width() - b.width();
        let dh = a.height() - b.height();
        (dx * dx + dy * dy + dw * dw + dh * dh).sqrt()
    });
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    let a = keyframes[lo].value();
    let b = keyframes[hi].value();
    NonZeroRect::from_xywh(
        lerp(a.x(), b.x(), t),
        lerp(a.y(), b.y(), t),
        lerp(a.width(), b.width(), t),
        lerp(a.height(), b.height(), t),
    )
}

// --- Transform tracks -------------------------------------------------------

/// Samples a SMIL or CSS transform track into a matrix.
fn sample_transform(track: &TransformTrack, easing: &Easing, progress: f32) -> Option<Transform> {
    match track {
        TransformTrack::Smil { kind, keyframes } => {
            sample_smil_transform(*kind, keyframes, easing, progress)
        }
        TransformTrack::Css { keyframes, .. } => sample_css_transform(keyframes, easing, progress),
    }
}

/// Samples a SMIL transform by lerping its typed parameters, then builds the
/// matrix from the interpolated parameters.
fn sample_smil_transform(
    kind: TransformKind,
    keyframes: &[Keyframe<Vec<f32>>],
    easing: &Easing,
    progress: f32,
) -> Option<Transform> {
    let paced = if matches!(easing.calc_mode(), CalcMode::Paced) {
        smil_paced_distances(kind, keyframes)
    } else {
        None
    };
    let (lo, hi, t) = locate_track(keyframes, easing, progress, paced)?;
    Some(build_smil_matrix(
        kind,
        keyframes[lo].value(),
        keyframes[hi].value(),
        t,
    ))
}

/// Computes the per-kind paced metric for a SMIL transform.
///
/// `rotate` has a defined metric only when its center is constant across the
/// track; a varying center returns `None`, and the caller falls back to linear.
fn smil_paced_distances(kind: TransformKind, keyframes: &[Keyframe<Vec<f32>>]) -> Option<Vec<f32>> {
    match kind {
        TransformKind::Translate => Some(segment_metrics(keyframes, |a, b| {
            let dx = param(a, 0, 0.0) - param(b, 0, 0.0);
            let dy = param(a, 1, 0.0) - param(b, 1, 0.0);
            (dx * dx + dy * dy).sqrt()
        })),
        TransformKind::Scale => Some(segment_metrics(keyframes, |a, b| {
            let ax = param(a, 0, 1.0);
            let bx = param(b, 0, 1.0);
            let dx = ax - bx;
            let dy = param(a, 1, ax) - param(b, 1, bx);
            (dx * dx + dy * dy).sqrt()
        })),
        TransformKind::SkewX | TransformKind::SkewY => Some(segment_metrics(keyframes, |a, b| {
            (param(a, 0, 0.0) - param(b, 0, 0.0)).abs()
        })),
        TransformKind::Rotate => rotate_center_constant(keyframes).then(|| {
            segment_metrics(keyframes, |a, b| (param(a, 0, 0.0) - param(b, 0, 0.0)).abs())
        }),
    }
}

/// Reports whether every rotate keyframe shares one center.
fn rotate_center_constant(keyframes: &[Keyframe<Vec<f32>>]) -> bool {
    let mut iter = keyframes.iter();
    let Some(first) = iter.next() else {
        return true;
    };
    let cx = param(first.value(), 1, 0.0);
    let cy = param(first.value(), 2, 0.0);
    iter.all(|k| {
        (param(k.value(), 1, 0.0) - cx).abs() < f32::EPSILON
            && (param(k.value(), 2, 0.0) - cy).abs() < f32::EPSILON
    })
}

/// Reads a transform parameter, falling back to `default` when absent.
fn param(values: &[f32], index: usize, default: f32) -> f32 {
    values.get(index).copied().unwrap_or(default)
}

/// Builds a transform matrix from two parameter lists lerped at `t`.
fn build_smil_matrix(kind: TransformKind, a: &[f32], b: &[f32], t: f32) -> Transform {
    match kind {
        TransformKind::Translate => {
            let tx = lerp(param(a, 0, 0.0), param(b, 0, 0.0), t);
            let ty = lerp(param(a, 1, 0.0), param(b, 1, 0.0), t);
            Transform::from_translate(tx, ty)
        }
        TransformKind::Scale => {
            let ax = param(a, 0, 1.0);
            let bx = param(b, 0, 1.0);
            let sx = lerp(ax, bx, t);
            let sy = lerp(param(a, 1, ax), param(b, 1, bx), t);
            Transform::from_scale(sx, sy)
        }
        TransformKind::Rotate => {
            let angle = lerp(param(a, 0, 0.0), param(b, 0, 0.0), t);
            let cx = lerp(param(a, 1, 0.0), param(b, 1, 0.0), t);
            let cy = lerp(param(a, 2, 0.0), param(b, 2, 0.0), t);
            Transform::from_rotate_at(angle, cx, cy)
        }
        TransformKind::SkewX => {
            let angle = lerp(param(a, 0, 0.0), param(b, 0, 0.0), t);
            Transform::from_skew(angle.to_radians().tan(), 0.0)
        }
        TransformKind::SkewY => {
            let angle = lerp(param(a, 0, 0.0), param(b, 0, 0.0), t);
            Transform::from_skew(0.0, angle.to_radians().tan())
        }
    }
}

/// Samples a CSS transform track.
///
/// When every keyframe shares one function-type signature the lists lerp
/// per function; otherwise the animation steps discretely and warns, since a
/// matrix decomposition of mismatched lists is out of scope.
fn sample_css_transform(
    keyframes: &[Keyframe<Vec<TransformFunction>>],
    easing: &Easing,
    progress: f32,
) -> Option<Transform> {
    if keyframes.is_empty() {
        return None;
    }

    if css_functions_compatible(keyframes) {
        let (lo, hi, t) = locate_track(keyframes, easing, progress, None)?;
        let functions: Vec<TransformFunction> = keyframes[lo]
            .value()
            .iter()
            .zip(keyframes[hi].value().iter())
            .map(|(a, b)| lerp_function(a, b, t))
            .collect();
        Some(build_css_matrix(&functions))
    } else {
        warn_incompatible_transform();
        let offsets: Vec<f32> = keyframes.iter().map(|k| k.offset().get()).collect();
        let index = discrete_index(&offsets, progress.clamp(0.0, 1.0));
        Some(build_css_matrix(keyframes[index].value()))
    }
}

/// Reports whether all keyframes share one function-type signature.
fn css_functions_compatible(keyframes: &[Keyframe<Vec<TransformFunction>>]) -> bool {
    let signature =
        |functions: &[TransformFunction]| functions.iter().map(std::mem::discriminant).collect();
    let mut iter = keyframes.iter();
    let Some(first) = iter.next() else {
        return true;
    };
    let expected: Vec<_> = signature(first.value());
    iter.all(|k| signature(k.value()) == expected)
}

/// Lerps two transform functions of the same variant.
fn lerp_function(a: &TransformFunction, b: &TransformFunction, t: f32) -> TransformFunction {
    use TransformFunction::*;
    match (a, b) {
        (Matrix(a0, a1, a2, a3, a4, a5), Matrix(b0, b1, b2, b3, b4, b5)) => Matrix(
            lerp(*a0, *b0, t),
            lerp(*a1, *b1, t),
            lerp(*a2, *b2, t),
            lerp(*a3, *b3, t),
            lerp(*a4, *b4, t),
            lerp(*a5, *b5, t),
        ),
        (Translate(ax, ay), Translate(bx, by)) => Translate(lerp(*ax, *bx, t), lerp(*ay, *by, t)),
        (TranslateX(a), TranslateX(b)) => TranslateX(lerp(*a, *b, t)),
        (TranslateY(a), TranslateY(b)) => TranslateY(lerp(*a, *b, t)),
        (Scale(ax, ay), Scale(bx, by)) => Scale(lerp(*ax, *bx, t), lerp(*ay, *by, t)),
        (ScaleX(a), ScaleX(b)) => ScaleX(lerp(*a, *b, t)),
        (ScaleY(a), ScaleY(b)) => ScaleY(lerp(*a, *b, t)),
        (Rotate(a), Rotate(b)) => Rotate(lerp(*a, *b, t)),
        (SkewX(a), SkewX(b)) => SkewX(lerp(*a, *b, t)),
        (SkewY(a), SkewY(b)) => SkewY(lerp(*a, *b, t)),
        _ => *a,
    }
}

/// Composes a function list into a single matrix, left to right.
fn build_css_matrix(functions: &[TransformFunction]) -> Transform {
    let mut matrix = Transform::identity();
    for function in functions {
        matrix = matrix.pre_concat(function_matrix(function));
    }
    matrix
}

/// Builds the matrix of one CSS transform function.
fn function_matrix(function: &TransformFunction) -> Transform {
    use TransformFunction::*;
    match *function {
        Matrix(a, b, c, d, e, f) => Transform::from_row(a, b, c, d, e, f),
        Translate(x, y) => Transform::from_translate(x, y),
        TranslateX(x) => Transform::from_translate(x, 0.0),
        TranslateY(y) => Transform::from_translate(0.0, y),
        Scale(x, y) => Transform::from_scale(x, y),
        ScaleX(x) => Transform::from_scale(x, 1.0),
        ScaleY(y) => Transform::from_scale(1.0, y),
        Rotate(angle) => Transform::from_rotate(angle),
        SkewX(angle) => Transform::from_skew(angle.to_radians().tan(), 0.0),
        SkewY(angle) => Transform::from_skew(0.0, angle.to_radians().tan()),
    }
}

// --- Path tracks ------------------------------------------------------------

/// Samples a baked path track point-wise, returning the shape and its
/// renderability.
///
/// The sampled shape renders unless both bracketing keyframes are degenerate or
/// the frame rests exactly on a degenerate keyframe with no progress toward a
/// renderable neighbor, so a `0 -> 100` grow draws at every `t > 0`.
fn sample_path(track: &PathTrack, easing: &Easing, progress: f32) -> Option<(Arc<Path>, bool)> {
    let keyframes = track.keyframes();
    if keyframes.is_empty() {
        return None;
    }

    let offsets: Vec<f32> = keyframes.iter().map(|k| k.offset().get()).collect();
    let timings: Vec<Option<TimingFunction>> =
        keyframes.iter().map(|k| k.timing_function().copied()).collect();
    let paced = if matches!(easing.calc_mode(), CalcMode::Paced) {
        Some(
            (0..keyframes.len().saturating_sub(1))
                .map(|i| path_distance(keyframes[i].path(), keyframes[i + 1].path()))
                .collect::<Vec<f32>>(),
        )
    } else {
        None
    };

    let (lo, hi, t) = locate(&offsets, &timings, easing, progress, paced.as_deref());

    let low_renderable = keyframes[lo].renderable();
    let high_renderable = keyframes[hi].renderable();
    let both_degenerate = !low_renderable && !high_renderable;
    let on_degenerate_start = t == 0.0 && !low_renderable;
    let renderable = !both_degenerate && !on_degenerate_start;

    let path = lerp_paths(keyframes[lo].path(), keyframes[hi].path(), t)?;
    Some((Arc::new(path), renderable))
}

/// The summed point-to-point distance between two verb-matched paths.
fn path_distance(a: &Path, b: &Path) -> f32 {
    let a_points = a.points();
    let b_points = b.points();
    let len = a_points.len().min(b_points.len());
    (0..len)
        .map(|i| {
            let dx = a_points[i].x - b_points[i].x;
            let dy = a_points[i].y - b_points[i].y;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

/// Builds the point-wise interpolation of two verb-matched paths at `t`.
fn lerp_paths(a: &Path, b: &Path, t: f32) -> Option<Path> {
    let mut builder = PathBuilder::new();
    let mut a_iter = a.segments();
    let mut b_iter = b.segments();
    loop {
        match (a_iter.next(), b_iter.next()) {
            (Some(a_seg), Some(b_seg)) => match (a_seg, b_seg) {
                (PathSegment::MoveTo(ap), PathSegment::MoveTo(bp)) => {
                    builder.move_to(lerp(ap.x, bp.x, t), lerp(ap.y, bp.y, t));
                }
                (PathSegment::LineTo(ap), PathSegment::LineTo(bp)) => {
                    builder.line_to(lerp(ap.x, bp.x, t), lerp(ap.y, bp.y, t));
                }
                (PathSegment::QuadTo(ac, ap), PathSegment::QuadTo(bc, bp)) => {
                    builder.quad_to(
                        lerp(ac.x, bc.x, t),
                        lerp(ac.y, bc.y, t),
                        lerp(ap.x, bp.x, t),
                        lerp(ap.y, bp.y, t),
                    );
                }
                (PathSegment::CubicTo(ac1, ac2, ap), PathSegment::CubicTo(bc1, bc2, bp)) => {
                    builder.cubic_to(
                        lerp(ac1.x, bc1.x, t),
                        lerp(ac1.y, bc1.y, t),
                        lerp(ac2.x, bc2.x, t),
                        lerp(ac2.y, bc2.y, t),
                        lerp(ap.x, bp.x, t),
                        lerp(ap.y, bp.y, t),
                    );
                }
                (PathSegment::Close, PathSegment::Close) => builder.close(),
                _ => return None,
            },
            (None, None) => break,
            _ => return None,
        }
    }
    builder.finish()
}

// --- Motion tracks ----------------------------------------------------------

/// Flatness tolerance in pixels for curve subdivision.
const FLATNESS: f32 = 0.1;
/// Maximum recursion depth for adaptive curve subdivision.
const MAX_DEPTH: u8 = 16;

/// A cumulative arc-length table over a flattened motion path.
struct ArcLength {
    points: Vec<Point>,
    cumulative: Vec<f32>,
    total: f32,
}

impl ArcLength {
    /// Flattens `path` and builds its cumulative arc-length table.
    ///
    /// Returns `None` for a path with no drawable length.
    fn build(path: &Path) -> Option<ArcLength> {
        let mut points: Vec<Point> = Vec::new();
        let mut cumulative: Vec<f32> = Vec::new();
        let mut total = 0.0;
        let mut current = Point::from_xy(0.0, 0.0);
        let mut subpath_start = Point::from_xy(0.0, 0.0);

        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    // A move introduces a gap: the point advances with no length.
                    points.push(p);
                    cumulative.push(total);
                    current = p;
                    subpath_start = p;
                }
                PathSegment::LineTo(p) => {
                    total += distance(current, p);
                    points.push(p);
                    cumulative.push(total);
                    current = p;
                }
                PathSegment::QuadTo(c, p) => {
                    let mut flattened = Vec::new();
                    flatten_quad(current, c, p, 0, &mut flattened);
                    for point in flattened {
                        total += distance(current, point);
                        points.push(point);
                        cumulative.push(total);
                        current = point;
                    }
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    let mut flattened = Vec::new();
                    flatten_cubic(current, c1, c2, p, 0, &mut flattened);
                    for point in flattened {
                        total += distance(current, point);
                        points.push(point);
                        cumulative.push(total);
                        current = point;
                    }
                }
                PathSegment::Close => {
                    total += distance(current, subpath_start);
                    points.push(subpath_start);
                    cumulative.push(total);
                    current = subpath_start;
                }
            }
        }

        if points.len() < 2 || total <= 0.0 {
            return None;
        }
        Some(ArcLength {
            points,
            cumulative,
            total,
        })
    }

    /// Returns the point and tangent angle (in degrees) at `distance_along`.
    fn sample(&self, distance_along: f32) -> (Point, f32) {
        let target = distance_along.clamp(0.0, self.total);
        let segment = self.segment_index(target);
        let start = self.cumulative[segment];
        let span = self.cumulative[segment + 1] - start;
        let local = if span > 0.0 {
            (target - start) / span
        } else {
            0.0
        };
        let p0 = self.points[segment];
        let p1 = self.points[segment + 1];
        let point = Point::from_xy(lerp(p0.x, p1.x, local), lerp(p0.y, p1.y, local));
        (point, tangent_angle(p0, p1))
    }

    /// Finds the polyline segment containing `target`.
    fn segment_index(&self, target: f32) -> usize {
        let count = self.cumulative.len();
        let found = self.cumulative.partition_point(|&c| c <= target);
        found.saturating_sub(1).min(count - 2)
    }
}

/// Samples an `animateMotion` track into its local transform.
fn sample_motion(track: &MotionTrack, easing: &Easing, progress: f32) -> Option<Transform> {
    let table = ArcLength::build(track.path())?;
    let fraction = motion_fraction(track, easing, progress.clamp(0.0, 1.0));
    let (point, tangent) = table.sample(fraction * table.total);
    let angle = match track.rotate() {
        MotionRotate::Auto => tangent,
        MotionRotate::AutoReverse => tangent + 180.0,
        MotionRotate::Angle(fixed) => fixed,
    };
    Some(Transform::from_translate(point.x, point.y).pre_concat(Transform::from_rotate(angle)))
}

/// Maps `progress` onto a path fraction in `0.0..=1.0`.
///
/// With `keyPoints` the fraction is looked up through the paired `keyTimes`
/// (spline-eased when requested); otherwise the fraction follows progress
/// directly, which under the default `paced` mode yields constant velocity.
fn motion_fraction(track: &MotionTrack, easing: &Easing, progress: f32) -> f32 {
    match track.key_points() {
        Some(key_points) if key_points.len() >= 2 => {
            let offsets: Vec<f32> = match easing.key_times() {
                Some(times) if times.len() == key_points.len() => {
                    times.iter().map(|t| t.get()).collect()
                }
                _ => uniform_offsets(key_points.len()),
            };
            let (lo, hi, local) = bracket(&offsets, progress);
            let eased = match easing.calc_mode() {
                CalcMode::Spline => easing
                    .key_splines()
                    .and_then(|splines| splines.get(lo))
                    .map(|spline| key_spline(*spline, local))
                    .unwrap_or(local),
                _ => local,
            };
            lerp(key_points[lo].get(), key_points[hi].get(), eased).clamp(0.0, 1.0)
        }
        _ => match easing.calc_mode() {
            CalcMode::Spline => easing
                .key_splines()
                .and_then(|splines| splines.first())
                .map(|spline| key_spline(*spline, progress))
                .unwrap_or(progress),
            _ => progress,
        },
    }
}

/// Evenly spaces `count` offsets across `0.0..=1.0`.
fn uniform_offsets(count: usize) -> Vec<f32> {
    if count <= 1 {
        return vec![0.0];
    }
    (0..count).map(|i| i as f32 / (count as f32 - 1.0)).collect()
}

/// The Euclidean distance between two points.
fn distance(a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

/// The tangent direction from `a` to `b`, in degrees.
fn tangent_angle(a: Point, b: Point) -> f32 {
    (b.y - a.y).atan2(b.x - a.x).to_degrees()
}

/// The midpoint of two points.
fn midpoint(a: Point, b: Point) -> Point {
    Point::from_xy((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

/// Adaptively flattens a quadratic curve, appending points up to `p2`.
fn flatten_quad(p0: Point, p1: Point, p2: Point, depth: u8, out: &mut Vec<Point>) {
    if depth >= MAX_DEPTH || perpendicular_distance(p0, p2, p1) <= FLATNESS {
        out.push(p2);
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p012 = midpoint(p01, p12);
    flatten_quad(p0, p01, p012, depth + 1, out);
    flatten_quad(p012, p12, p2, depth + 1, out);
}

/// Adaptively flattens a cubic curve, appending points up to `p3`.
fn flatten_cubic(p0: Point, p1: Point, p2: Point, p3: Point, depth: u8, out: &mut Vec<Point>) {
    let flat = perpendicular_distance(p0, p3, p1) <= FLATNESS
        && perpendicular_distance(p0, p3, p2) <= FLATNESS;
    if depth >= MAX_DEPTH || flat {
        out.push(p3);
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let p0123 = midpoint(p012, p123);
    flatten_cubic(p0, p01, p012, p0123, depth + 1, out);
    flatten_cubic(p0123, p123, p23, p3, depth + 1, out);
}

/// The perpendicular distance of `p` from the line through `a` and `b`.
fn perpendicular_distance(a: Point, b: Point, p: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length < f32::EPSILON {
        return distance(a, p);
    }
    ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / length
}

// --- Warnings ---------------------------------------------------------------

fn warn_incompatible_transform() {
    log::warn!("Unsupported transform animation; using discrete interpolation.");
}

fn warn_paced_unsupported() {
    log::warn!("Paced interpolation is not supported here; using linear.");
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::sync::Once;

    use usvg::{Track, TransformOrigin, TransformOriginValue};

    // A thread-local capture buffer keeps each test's warnings isolated even
    // when the suite runs in parallel against the one global logger.
    thread_local! {
        static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    struct Capture;

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            WARNINGS.with(|w| w.borrow_mut().push(record.args().to_string()));
        }
        fn flush(&self) {}
    }

    fn init_logger() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = log::set_boxed_logger(Box::new(Capture));
            log::set_max_level(log::LevelFilter::Warn);
        });
    }

    fn clear_warnings() {
        WARNINGS.with(|w| w.borrow_mut().clear());
    }

    fn warned(literal: &str) -> bool {
        WARNINGS.with(|w| w.borrow().iter().any(|m| m == literal))
    }

    fn n(v: f32) -> NormalizedF32 {
        NormalizedF32::new_clamped(v)
    }

    fn linear() -> Easing {
        Easing::new(CalcMode::Linear, None, None)
    }

    fn paced() -> Easing {
        Easing::new(CalcMode::Paced, None, None)
    }

    fn discrete() -> Easing {
        Easing::new(CalcMode::Discrete, None, None)
    }

    fn scalar_track(values: &[(f32, f32)]) -> Track<f32> {
        Track::new(
            values
                .iter()
                .map(|&(offset, value)| Keyframe::new(n(offset), value, None))
                .collect(),
        )
    }

    fn approx(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() < tol, "expected {b}, got {a}");
    }

    fn expect_scalar(value: Option<SampledValue>) -> f32 {
        match value {
            Some(SampledValue::StrokeWidth(v)) => v,
            other => panic!("expected a stroke width, got {other:?}"),
        }
    }

    fn expect_transform(value: Option<SampledValue>) -> Transform {
        match value {
            Some(SampledValue::Transform(t)) => t,
            other => panic!("expected a transform, got {other:?}"),
        }
    }

    fn approx_transform(a: Transform, b: Transform, tol: f32) {
        approx(a.sx, b.sx, tol);
        approx(a.ky, b.ky, tol);
        approx(a.kx, b.kx, tol);
        approx(a.sy, b.sy, tol);
        approx(a.tx, b.tx, tol);
        approx(a.ty, b.ty, tol);
    }

    #[test]
    fn scalar_linear_midpoint() {
        let kind = AnimationKind::StrokeWidth(scalar_track(&[(0.0, 0.0), (1.0, 100.0)]));
        approx(
            expect_scalar(interpolate_track(&kind, &linear(), 0.25)),
            25.0,
            1e-4,
        );
    }

    #[test]
    fn single_keyframe_is_constant() {
        // A `set`-style single-keyframe track holds its one value everywhere.
        let kind = AnimationKind::StrokeWidth(scalar_track(&[(0.0, 7.0)]));
        approx(expect_scalar(interpolate_track(&kind, &linear(), 0.0)), 7.0, 1e-4);
        approx(expect_scalar(interpolate_track(&kind, &linear(), 0.5)), 7.0, 1e-4);
        approx(expect_scalar(interpolate_track(&kind, &linear(), 1.0)), 7.0, 1e-4);
    }

    #[test]
    fn paced_scalar_spacing() {
        // Distances 10 and 90 across [0,10,100]; time follows distance, not the
        // uniform offsets that linear interpolation would honor.
        let kind =
            AnimationKind::StrokeWidth(scalar_track(&[(0.0, 0.0), (0.5, 10.0), (1.0, 100.0)]));
        let easing = paced();

        approx(expect_scalar(interpolate_track(&kind, &easing, 0.05)), 5.0, 1e-3);
        approx(expect_scalar(interpolate_track(&kind, &easing, 0.55)), 55.0, 1e-3);

        // The linear reading at the same progress differs, proving paced spacing.
        approx(expect_scalar(interpolate_track(&kind, &linear(), 0.05)), 1.0, 1e-3);
    }

    #[test]
    fn spline_easing_between_keyframes() {
        // An ease-in spline slows the segment start, so the value trails linear.
        let spline = [0.42, 0.0, 1.0, 1.0];
        let easing = Easing::new(CalcMode::Spline, None, Some(vec![spline]));
        let kind = AnimationKind::StrokeWidth(scalar_track(&[(0.0, 0.0), (1.0, 100.0)]));

        let value = expect_scalar(interpolate_track(&kind, &easing, 0.3));
        let expected = 100.0 * super::super::easing::cubic_bezier(0.42, 0.0, 1.0, 1.0, 0.3);
        approx(value, expected, 1e-3);
        assert!(value < 30.0, "ease-in should trail linear, got {value}");
    }

    #[test]
    fn discrete_stepping_holds_last() {
        // Values step on half-open intervals and the last holds past its offset.
        let kind = AnimationKind::StrokeWidth(scalar_track(&[
            (0.0, 10.0),
            (0.4, 20.0),
            (0.8, 30.0),
        ]));
        let easing = discrete();

        approx(expect_scalar(interpolate_track(&kind, &easing, 0.0)), 10.0, 1e-4);
        approx(expect_scalar(interpolate_track(&kind, &easing, 0.39)), 10.0, 1e-4);
        approx(expect_scalar(interpolate_track(&kind, &easing, 0.4)), 20.0, 1e-4);
        approx(expect_scalar(interpolate_track(&kind, &easing, 0.79)), 20.0, 1e-4);
        approx(expect_scalar(interpolate_track(&kind, &easing, 0.8)), 30.0, 1e-4);
        // The last value holds from 0.8 to the end of the simple duration.
        approx(expect_scalar(interpolate_track(&kind, &easing, 1.0)), 30.0, 1e-4);
    }

    #[test]
    fn smil_rotate_past_180_uses_param_lerp() {
        // 0 -> 270 lerps the angle to 135 at the midpoint, not the -45 a
        // shortest-arc matrix blend would take.
        let keyframes = vec![
            Keyframe::new(n(0.0), vec![0.0, 0.0, 0.0], None),
            Keyframe::new(n(1.0), vec![270.0, 0.0, 0.0], None),
        ];
        let kind = AnimationKind::Transform(TransformTrack::Smil {
            kind: TransformKind::Rotate,
            keyframes,
        });

        let sampled = expect_transform(interpolate_track(&kind, &linear(), 0.5));
        approx_transform(sampled, Transform::from_rotate_at(135.0, 0.0, 0.0), 1e-4);
        // The shortest-arc blend (-45) would give the opposite-sign shear.
        assert!(sampled.ky > 0.0, "135deg keeps a positive sin, got {}", sampled.ky);
    }

    #[test]
    fn paced_rotate_spacing_by_angle() {
        // Angle deltas 90 and 270 across [0,90,360] with a constant center.
        let keyframes = vec![
            Keyframe::new(n(0.0), vec![0.0, 0.0, 0.0], None),
            Keyframe::new(n(0.5), vec![90.0, 0.0, 0.0], None),
            Keyframe::new(n(1.0), vec![360.0, 0.0, 0.0], None),
        ];
        let kind = AnimationKind::Transform(TransformTrack::Smil {
            kind: TransformKind::Rotate,
            keyframes,
        });

        // At quarter distance the angle is exactly 90 (end of the first delta).
        let at_quarter = expect_transform(interpolate_track(&kind, &paced(), 0.25));
        approx_transform(at_quarter, Transform::from_rotate_at(90.0, 0.0, 0.0), 1e-3);
        // Linear at the same progress would only reach 45 degrees.
        let linear_quarter = expect_transform(interpolate_track(&kind, &linear(), 0.25));
        approx_transform(linear_quarter, Transform::from_rotate_at(45.0, 0.0, 0.0), 1e-3);
    }

    #[test]
    fn paced_rotate_varying_center_falls_back_to_linear() {
        init_logger();
        clear_warnings();

        let keyframes = vec![
            Keyframe::new(n(0.0), vec![0.0, 0.0, 0.0], None),
            Keyframe::new(n(1.0), vec![90.0, 10.0, 10.0], None),
        ];
        let kind = AnimationKind::Transform(TransformTrack::Smil {
            kind: TransformKind::Rotate,
            keyframes,
        });

        // A varying center has no principled paced metric: fall back to linear.
        let sampled = expect_transform(interpolate_track(&kind, &paced(), 0.5));
        approx_transform(sampled, Transform::from_rotate_at(45.0, 5.0, 5.0), 1e-3);
        assert!(warned("Paced interpolation is not supported here; using linear."));
    }

    #[test]
    fn css_compatible_transform_lerps() {
        let keyframes = vec![
            Keyframe::new(n(0.0), vec![TransformFunction::Translate(0.0, 0.0)], None),
            Keyframe::new(n(1.0), vec![TransformFunction::Translate(100.0, 0.0)], None),
        ];
        let kind = AnimationKind::Transform(TransformTrack::Css {
            keyframes,
            origin: TransformOrigin::new(
                TransformOriginValue::Percent(50.0),
                TransformOriginValue::Percent(50.0),
            ),
            box_: usvg::TransformBox::ViewBox,
        });

        let sampled = expect_transform(interpolate_track(&kind, &linear(), 0.5));
        approx_transform(sampled, Transform::from_translate(50.0, 0.0), 1e-4);
    }

    #[test]
    fn css_incompatible_transform_steps_and_warns() {
        init_logger();
        clear_warnings();

        let keyframes = vec![
            Keyframe::new(n(0.0), vec![TransformFunction::Translate(0.0, 0.0)], None),
            Keyframe::new(n(1.0), vec![TransformFunction::Scale(2.0, 2.0)], None),
        ];
        let kind = AnimationKind::Transform(TransformTrack::Css {
            keyframes,
            origin: TransformOrigin::new(
                TransformOriginValue::Percent(50.0),
                TransformOriginValue::Percent(50.0),
            ),
            box_: usvg::TransformBox::ViewBox,
        });

        // Structurally incompatible lists step to the low keyframe (translate 0).
        let sampled = expect_transform(interpolate_track(&kind, &linear(), 0.5));
        approx_transform(sampled, Transform::from_translate(0.0, 0.0), 1e-4);
        assert!(warned("Unsupported transform animation; using discrete interpolation."));
    }

    #[test]
    fn color_channels_lerp() {
        let track = Track::new(vec![
            Keyframe::new(n(0.0), Color::new_rgba(255, 0, 0, 255), None),
            Keyframe::new(n(1.0), Color::new_rgba(0, 0, 255, 255), None),
        ]);
        let kind = AnimationKind::Fill(track);
        match interpolate_track(&kind, &linear(), 0.5) {
            Some(SampledValue::Color(c)) => {
                assert_eq!(c.red, 128);
                assert_eq!(c.green, 0);
                assert_eq!(c.blue, 128);
                assert_eq!(c.alpha, 255);
            }
            other => panic!("expected a color, got {other:?}"),
        }
    }

    #[test]
    fn dasharray_lerps_element_wise() {
        let track = Track::new(vec![
            Keyframe::new(n(0.0), vec![4.0, 4.0], None),
            Keyframe::new(n(1.0), vec![8.0, 12.0], None),
        ]);
        let kind = AnimationKind::StrokeDasharray(track);
        match interpolate_track(&kind, &linear(), 0.5) {
            Some(SampledValue::StrokeDasharray(dashes)) => {
                approx(dashes[0], 6.0, 1e-4);
                approx(dashes[1], 8.0, 1e-4);
            }
            other => panic!("expected a dasharray, got {other:?}"),
        }
    }

    #[test]
    fn viewbox_components_lerp() {
        let a = NonZeroRect::from_xywh(0.0, 0.0, 10.0, 10.0).unwrap();
        let b = NonZeroRect::from_xywh(0.0, 0.0, 20.0, 30.0).unwrap();
        let track = Track::new(vec![
            Keyframe::new(n(0.0), a, None),
            Keyframe::new(n(1.0), b, None),
        ]);
        let kind = AnimationKind::ViewBox(track);
        match interpolate_track(&kind, &linear(), 0.5) {
            Some(SampledValue::ViewBox(rect)) => {
                approx(rect.width(), 15.0, 1e-4);
                approx(rect.height(), 20.0, 1e-4);
            }
            other => panic!("expected a view box, got {other:?}"),
        }
    }

    #[test]
    fn discrete_enum_steps() {
        let track = Track::new(vec![
            Keyframe::new(n(0.0), LineCap::Butt, None),
            Keyframe::new(n(0.5), LineCap::Round, None),
        ]);
        let kind = AnimationKind::StrokeLinecap(track);
        match interpolate_track(&kind, &discrete(), 0.6) {
            Some(SampledValue::StrokeLinecap(cap)) => assert_eq!(cap, LineCap::Round),
            other => panic!("expected a line cap, got {other:?}"),
        }
    }

    fn rect_path(x: f32, y: f32, width: f32, height: f32) -> Arc<Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(x, y);
        builder.line_to(x + width, y);
        builder.line_to(x + width, y + height);
        builder.line_to(x, y + height);
        builder.close();
        Arc::new(builder.finish().unwrap())
    }

    #[test]
    fn path_grow_from_zero() {
        // A degenerate (zero-width) first keyframe with matching verbs grows into
        // a real rectangle: renderable at every t > 0, invisible only at t = 0.
        let degenerate = PathKeyframe::new(n(0.0), rect_path(10.0, 10.0, 0.0, 20.0), false, None);
        let real = PathKeyframe::new(n(1.0), rect_path(10.0, 10.0, 40.0, 20.0), true, None);
        let kind = AnimationKind::Path(PathTrack::new(vec![degenerate, real], None));

        let mid = interpolate_track(&kind, &linear(), 0.5);
        let (mid_path, mid_renderable) = match mid {
            Some(SampledValue::Path(path, renderable)) => (path, renderable),
            other => panic!("expected a path, got {other:?}"),
        };
        assert!(mid_renderable, "midpoint of a grow must render");

        // The point-wise midpoint equals the geometry built at the mid width (20).
        let expected = rect_path(10.0, 10.0, 20.0, 20.0);
        let sampled_points = mid_path.points();
        let expected_points = expected.points();
        assert_eq!(sampled_points.len(), expected_points.len());
        for (got, want) in sampled_points.iter().zip(expected_points.iter()) {
            approx(got.x, want.x, 1e-4);
            approx(got.y, want.y, 1e-4);
        }

        // Exactly at t = 0 the frame rests on the degenerate keyframe.
        match interpolate_track(&kind, &linear(), 0.0) {
            Some(SampledValue::Path(_, renderable)) => {
                assert!(!renderable, "the degenerate start must not render");
            }
            other => panic!("expected a path, got {other:?}"),
        }
    }

    // Needs the PathKeyframe constructor.
    use usvg::PathKeyframe;

    fn line_path(points: &[(f32, f32)]) -> Arc<Path> {
        let mut builder = PathBuilder::new();
        let (x0, y0) = points[0];
        builder.move_to(x0, y0);
        for &(x, y) in &points[1..] {
            builder.line_to(x, y);
        }
        Arc::new(builder.finish().unwrap())
    }

    fn cubic_path() -> Arc<Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.cubic_to(0.0, 100.0, 100.0, 100.0, 100.0, 0.0);
        Arc::new(builder.finish().unwrap())
    }

    /// A dense-sampling reference length of the fixed cubic above.
    fn cubic_reference_length() -> f32 {
        let steps = 20_000;
        let mut previous = Point::from_xy(0.0, 0.0);
        let mut total = 0.0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            // Bezier (0,0) (0,100) (100,100) (100,0).
            let x = 3.0 * mt * t * t * 100.0 + t * t * t * 100.0;
            let y = 3.0 * mt * mt * t * 100.0 + 3.0 * mt * t * t * 100.0;
            let point = Point::from_xy(x, y);
            total += distance(previous, point);
            previous = point;
        }
        total
    }

    #[test]
    fn motion_straight_baseline_length() {
        // Two axis-aligned segments: 100 across then 100 down = 200 exactly.
        let table = ArcLength::build(&line_path(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)]))
            .unwrap();
        approx(table.total, 200.0, 1e-3);
    }

    #[test]
    fn motion_quadratic_length_matches_reference() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.quad_to(50.0, 100.0, 100.0, 0.0);
        let path = builder.finish().unwrap();
        let table = ArcLength::build(&path).unwrap();

        let steps = 20_000;
        let mut previous = Point::from_xy(0.0, 0.0);
        let mut reference = 0.0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            let x = 2.0 * mt * t * 50.0 + t * t * 100.0;
            let y = 2.0 * mt * t * 100.0;
            let point = Point::from_xy(x, y);
            reference += distance(previous, point);
            previous = point;
        }
        assert!(
            (table.total - reference).abs() / reference < 0.005,
            "quadratic length {} vs reference {reference}",
            table.total
        );
    }

    #[test]
    fn motion_cubic_length_within_tolerance() {
        let table = ArcLength::build(&cubic_path()).unwrap();
        let reference = cubic_reference_length();
        assert!(
            (table.total - reference).abs() / reference < 0.005,
            "cubic length {} vs reference {reference}",
            table.total
        );
    }

    #[test]
    fn motion_midpoint_is_arc_length_centered() {
        // The fixed-angle motion transform carries the sampled point in tx/ty.
        let track = MotionTrack::new(cubic_path(), None, MotionRotate::Angle(0.0));
        let kind = AnimationKind::Motion(track);
        let sampled = match interpolate_track(&kind, &paced(), 0.5) {
            Some(SampledValue::Motion(t)) => t,
            other => panic!("expected a motion transform, got {other:?}"),
        };

        // The half-length point of this symmetric curve sits at its apex x = 50.
        let table = ArcLength::build(&cubic_path()).unwrap();
        let (expected, _) = table.sample(table.total * 0.5);
        approx(sampled.tx, expected.x, 1e-2);
        approx(sampled.ty, expected.y, 1e-2);
        approx(sampled.tx, 50.0, 1e-2);
    }

    #[test]
    fn motion_tangent_drives_auto_rotation() {
        // A 45-degree diagonal yields a 45-degree auto rotation at every point.
        let track = MotionTrack::new(line_path(&[(0.0, 0.0), (100.0, 100.0)]), None, MotionRotate::Auto);
        let kind = AnimationKind::Motion(track);
        let sampled = match interpolate_track(&kind, &paced(), 0.5) {
            Some(SampledValue::Motion(t)) => t,
            other => panic!("expected a motion transform, got {other:?}"),
        };
        // from_rotate(45) has sx = cos(45) = 0.7071.
        approx(sampled.sx, 45.0_f32.to_radians().cos(), 1e-3);
        approx(sampled.tx, 50.0, 1e-3);
        approx(sampled.ty, 50.0, 1e-3);
    }
}
