// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia::Transform;
use usvg::{
    CalcMode, Easing, Keyframe, TimingFunction, TransformFunction, TransformKind, TransformTrack,
};

use super::locate::{discrete_index, lerp, locate_track, segment_metrics};

/// Samples a SMIL or CSS transform track into a matrix.
pub(super) fn sample_transform(
    track: &TransformTrack,
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Transform> {
    match track {
        TransformTrack::Smil { kind, keyframes } => {
            sample_smil_transform(*kind, keyframes, easing, timing_function, progress)
        }
        TransformTrack::Css { keyframes, .. } => {
            sample_css_transform(keyframes, easing, timing_function, progress)
        }
    }
}

/// Samples a SMIL transform by lerping its typed parameters, then builds the
/// matrix from the interpolated parameters.
fn sample_smil_transform(
    kind: TransformKind,
    keyframes: &[Keyframe<Vec<f32>>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Transform> {
    let paced = if matches!(easing.calc_mode(), CalcMode::Paced) {
        smil_paced_distances(kind, keyframes)
    } else {
        None
    };
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
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
            segment_metrics(keyframes, |a, b| {
                (param(a, 0, 0.0) - param(b, 0, 0.0)).abs()
            })
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
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Transform> {
    if keyframes.is_empty() {
        return None;
    }

    if css_functions_compatible(keyframes) {
        let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, None)?;
        let functions: Vec<TransformFunction> = keyframes[lo]
            .value()
            .iter()
            .zip(keyframes[hi].value().iter())
            .map(|(a, b)| lerp_function(a, b, t))
            .collect();
        Some(build_css_matrix(&functions))
    } else {
        warn_incompatible_transform();
        let index = discrete_index(keyframes, progress.clamp(0.0, 1.0));
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

fn warn_incompatible_transform() {
    log::warn!("Unsupported transform animation; using discrete interpolation.");
}
