// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia::Transform;
use usvg::{CalcMode, Easing, Keyframe, TimingFunction, Track, TransformFunction};

use super::locate::{discrete_index, lerp, locate_track, segment_metrics};

/// Samples a canonical transform-function track into a matrix.
pub(super) fn sample_transform(
    track: &Track<Vec<TransformFunction>>,
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Transform> {
    sample_function_lists(track.keyframes(), easing, timing_function, progress)
}

/// Samples structurally compatible function lists or steps incompatible lists.
fn sample_function_lists(
    keyframes: &[Keyframe<Vec<TransformFunction>>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Transform> {
    if keyframes.is_empty() {
        return None;
    }

    if css_functions_compatible(keyframes) {
        let paced = matches!(easing.calc_mode(), CalcMode::Paced)
            .then(|| list_paced_distances(keyframes))
            .flatten();
        let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PacedSignature {
    Translate,
    Scale,
    SkewX,
    SkewY,
    Rotate,
    RotateAt,
    Other,
}

fn list_paced_distances(keyframes: &[Keyframe<Vec<TransformFunction>>]) -> Option<Vec<f32>> {
    let first = keyframes.first()?;
    let signature = paced_signature(first.value());
    if !keyframes
        .iter()
        .all(|keyframe| paced_signature(keyframe.value()) == signature)
    {
        return None;
    }
    match signature {
        PacedSignature::Translate => Some(segment_metrics(keyframes, |a, b| {
            distance(translate(a, 0), translate(b, 0))
        })),
        PacedSignature::Scale => Some(segment_metrics(keyframes, |a, b| {
            distance(scale(a), scale(b))
        })),
        PacedSignature::SkewX | PacedSignature::SkewY | PacedSignature::Rotate => {
            Some(segment_metrics(keyframes, |a, b| {
                (angle(a) - angle(b)).abs()
            }))
        }
        PacedSignature::RotateAt if wrapper_translations_constant(keyframes) => {
            Some(segment_metrics(keyframes, |a, b| {
                (angle_at(a) - angle_at(b)).abs()
            }))
        }
        PacedSignature::RotateAt | PacedSignature::Other => None,
    }
}

fn paced_signature(functions: &[TransformFunction]) -> PacedSignature {
    match functions {
        [TransformFunction::Translate(_, _)] => PacedSignature::Translate,
        [TransformFunction::Scale(_, _)] => PacedSignature::Scale,
        [TransformFunction::SkewX(_)] => PacedSignature::SkewX,
        [TransformFunction::SkewY(_)] => PacedSignature::SkewY,
        [TransformFunction::Rotate(_)] => PacedSignature::Rotate,
        [
            TransformFunction::Translate(_, _),
            TransformFunction::Rotate(_),
            TransformFunction::Translate(_, _),
        ] => PacedSignature::RotateAt,
        _ => PacedSignature::Other,
    }
}

fn wrapper_translations_constant(keyframes: &[Keyframe<Vec<TransformFunction>>]) -> bool {
    let Some(first) = keyframes.first() else {
        return true;
    };
    let before = translate(first.value(), 0);
    let after = translate(first.value(), 2);
    keyframes.iter().all(|keyframe| {
        close(translate(keyframe.value(), 0), before)
            && close(translate(keyframe.value(), 2), after)
    })
}

fn translate(functions: &[TransformFunction], index: usize) -> (f32, f32) {
    match functions[index] {
        TransformFunction::Translate(x, y) => (x, y),
        _ => unreachable!("paced signature guarantees a translate function"),
    }
}

fn scale(functions: &[TransformFunction]) -> (f32, f32) {
    match functions[0] {
        TransformFunction::Scale(x, y) => (x, y),
        _ => unreachable!("paced signature guarantees a scale function"),
    }
}

fn angle(functions: &[TransformFunction]) -> f32 {
    match functions[0] {
        TransformFunction::SkewX(value)
        | TransformFunction::SkewY(value)
        | TransformFunction::Rotate(value) => value,
        _ => unreachable!("paced signature guarantees an angular function"),
    }
}

fn angle_at(functions: &[TransformFunction]) -> f32 {
    match functions[1] {
        TransformFunction::Rotate(value) => value,
        _ => unreachable!("paced signature guarantees a middle rotate function"),
    }
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < f32::EPSILON && (a.1 - b.1).abs() < f32::EPSILON
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

pub(super) const INCOMPATIBLE_WARNING: &str =
    "Unsupported transform animation; using discrete interpolation.";

fn warn_incompatible_transform() {
    log::warn!("{INCOMPATIBLE_WARNING}");
}

#[cfg(test)]
#[path = "transform/tests.rs"]
mod tests;
