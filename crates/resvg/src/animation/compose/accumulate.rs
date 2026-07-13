// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use svgtypes::Color;
use tiny_skia::{Path, PathBuilder, PathSegment, Transform};
use usvg::{AnimationKind, Easing, TimingFunction};

use super::super::interpolate::{interpolate_track_with_timing, SampledValue};

/// Applies `accumulate="sum"` to a sampled value for iteration `iteration`.
pub(super) fn accumulate(
    kind: &AnimationKind,
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    sampled: SampledValue,
    iteration: u32,
) -> SampledValue {
    if is_discrete(&sampled) {
        warn_accumulate_ignored();
        return sampled;
    }
    if iteration == 0 {
        return sampled;
    }
    // Baked path tracks carry a precomputed per-iteration delta.
    if let (AnimationKind::Path(track), SampledValue::Path(path, renderable)) = (kind, &sampled) {
        return match track.accumulation_delta() {
            Some(delta) => accumulate_path(path, *renderable, delta, iteration),
            None => sampled,
        };
    }
    let Some(end) = interpolate_track_with_timing(kind, easing, timing_function, 1.0) else {
        return sampled;
    };
    let factor = iteration as f32;
    match (sampled, end) {
        (SampledValue::Transform(value), SampledValue::Transform(end)) => {
            SampledValue::Transform(accumulate_transform(value, end, iteration))
        }
        (SampledValue::Motion(value), SampledValue::Motion(end)) => {
            SampledValue::Motion(accumulate_transform(value, end, iteration))
        }
        (SampledValue::Opacity(value), SampledValue::Opacity(end)) => {
            SampledValue::Opacity((value + factor * end).clamp(0.0, 1.0))
        }
        (SampledValue::StrokeWidth(value), SampledValue::StrokeWidth(end)) => {
            SampledValue::StrokeWidth(value + factor * end)
        }
        (SampledValue::StrokeDashoffset(value), SampledValue::StrokeDashoffset(end)) => {
            SampledValue::StrokeDashoffset(value + factor * end)
        }
        (SampledValue::StrokeMiterlimit(value), SampledValue::StrokeMiterlimit(end)) => {
            SampledValue::StrokeMiterlimit(value + factor * end)
        }
        (SampledValue::GradientGeometry(value), SampledValue::GradientGeometry(end)) => {
            SampledValue::GradientGeometry(value + factor * end)
        }
        (SampledValue::ImageGeometry(value), SampledValue::ImageGeometry(end)) => {
            SampledValue::ImageGeometry(value + factor * end)
        }
        (SampledValue::Color(value), SampledValue::Color(end)) => {
            SampledValue::Color(add_color(value, end, iteration))
        }
        (SampledValue::StrokeDasharray(value), SampledValue::StrokeDasharray(end)) => {
            let len = value.len().min(end.len());
            SampledValue::StrokeDasharray((0..len).map(|i| value[i] + factor * end[i]).collect())
        }
        (sampled, _) => sampled,
    }
}

/// Whether a sampled value is a discrete or enumerated kind, which cannot
/// accumulate.
fn is_discrete(value: &SampledValue) -> bool {
    matches!(
        value,
        SampledValue::FillRule(_)
            | SampledValue::StrokeLinecap(_)
            | SampledValue::StrokeLinejoin(_)
            | SampledValue::Display(_)
            | SampledValue::Visibility(_)
    )
}

/// Post-multiplies `end` onto `value` once per completed iteration.
fn accumulate_transform(value: Transform, end: Transform, iteration: u32) -> Transform {
    let mut matrix = value;
    for _ in 0..iteration {
        matrix = matrix.pre_concat(end);
    }
    matrix
}

/// Adds `delta × times` onto `base`, saturating each 8-bit channel.
pub(super) fn add_color(base: Color, delta: Color, times: u32) -> Color {
    let add = |a: u8, b: u8| -> u8 { (u32::from(a) + u32::from(b) * times).min(255) as u8 };
    Color::new_rgba(
        add(base.red, delta.red),
        add(base.green, delta.green),
        add(base.blue, delta.blue),
        add(base.alpha, delta.alpha),
    )
}

/// Offsets each point of a baked path by `iteration × delta`, point-wise.
fn accumulate_path(
    path: &Arc<Path>,
    renderable: bool,
    delta: &Path,
    iteration: u32,
) -> SampledValue {
    let factor = iteration as f32;
    let mut builder = PathBuilder::new();
    let mut base = path.segments();
    let mut step = delta.segments();
    loop {
        match (base.next(), step.next()) {
            (Some(base_segment), Some(step_segment)) => {
                if !accumulate_segment(&mut builder, base_segment, step_segment, factor) {
                    return SampledValue::Path(path.clone(), renderable);
                }
            }
            (None, None) => break,
            _ => return SampledValue::Path(path.clone(), renderable),
        }
    }
    match builder.finish() {
        Some(result) => SampledValue::Path(Arc::new(result), renderable),
        None => SampledValue::Path(path.clone(), renderable),
    }
}

/// Adds one verb-matched segment offset; returns `false` on a verb mismatch.
fn accumulate_segment(
    builder: &mut PathBuilder,
    base: PathSegment,
    step: PathSegment,
    factor: f32,
) -> bool {
    match (base, step) {
        (PathSegment::MoveTo(b), PathSegment::MoveTo(s)) => {
            builder.move_to(b.x + factor * s.x, b.y + factor * s.y);
        }
        (PathSegment::LineTo(b), PathSegment::LineTo(s)) => {
            builder.line_to(b.x + factor * s.x, b.y + factor * s.y);
        }
        (PathSegment::QuadTo(bc, b), PathSegment::QuadTo(sc, s)) => {
            builder.quad_to(
                bc.x + factor * sc.x,
                bc.y + factor * sc.y,
                b.x + factor * s.x,
                b.y + factor * s.y,
            );
        }
        (PathSegment::CubicTo(bc1, bc2, b), PathSegment::CubicTo(sc1, sc2, s)) => {
            builder.cubic_to(
                bc1.x + factor * sc1.x,
                bc1.y + factor * sc1.y,
                bc2.x + factor * sc2.x,
                bc2.y + factor * sc2.y,
                b.x + factor * s.x,
                b.y + factor * s.y,
            );
        }
        (PathSegment::Close, PathSegment::Close) => builder.close(),
        _ => return false,
    }
    true
}

fn warn_accumulate_ignored() {
    log::warn!("Unsupported accumulate value; ignoring.");
}
