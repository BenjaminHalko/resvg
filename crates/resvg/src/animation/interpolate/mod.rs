// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-kind value interpolation for animation tracks.
//!
//! [`interpolate_track`] samples one [`usvg::AnimationKind`] at a normalized
//! progress within its simple duration and returns the typed value. Scalars and
//! opacities lerp linearly, colors lerp per sRGB channel, transform function
//! lists lerp only when structurally compatible (otherwise they step
//! discretely), baked path tracks lerp point-wise, and `animateMotion` maps
//! progress onto the path by arc length.
//!
//! The `calcMode` in [`usvg::Easing`] selects the segment behavior: `linear` and
//! `spline` interpolate between the two bracketing keyframes (splines and CSS
//! per-keyframe timing functions shape the segment parameter), `discrete` steps,
//! and `paced` spaces the keyframes by a per-kind distance metric.

use std::sync::Arc;

use svgtypes::Color;
use tiny_skia::{Path, Transform};
use usvg::{
    AnimationKind, AnimationVisibility, Easing, FillRule, LineCap, LineJoin, NonZeroRect,
    TimingFunction,
};

mod locate;
mod motion;
mod path;
mod scalar;
mod transform;

use motion::sample_motion;
use path::sample_path;
use scalar::{
    sample_color, sample_dasharray, sample_discrete, sample_discrete_before_boundary,
    sample_miterlimit, sample_opacity, sample_scalar, sample_viewbox,
};
use transform::sample_transform;

/// A single sampled animation value, typed by the track it came from.
///
/// CSS transform origins are baked into function lists after static bounds are
/// resolved. Stop offsets are reported as
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
    interpolate_track_with_timing(kind, easing, None, progress)
}

/// Interpolates a track with its animation-level CSS timing function.
pub(crate) fn interpolate_track_with_timing(
    kind: &AnimationKind,
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<SampledValue> {
    match kind {
        AnimationKind::Transform(track) | AnimationKind::GradientTransform(track) => {
            sample_transform(track, easing, timing_function, progress).map(SampledValue::Transform)
        }
        AnimationKind::Motion(track) => {
            sample_motion(track, easing, progress).map(SampledValue::Motion)
        }
        AnimationKind::Opacity(track) | AnimationKind::StopOpacity(track) => {
            sample_opacity(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::Opacity)
        }
        AnimationKind::Fill(track)
        | AnimationKind::Stroke(track)
        | AnimationKind::StopColor(track) => {
            sample_color(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::Color)
        }
        AnimationKind::StrokeWidth(track) => {
            sample_scalar(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::StrokeWidth)
        }
        AnimationKind::StrokeDashoffset(track) => {
            sample_scalar(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::StrokeDashoffset)
        }
        AnimationKind::StrokeDasharray(track) => {
            sample_dasharray(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::StrokeDasharray)
        }
        AnimationKind::StrokeMiterlimit(track) => {
            sample_miterlimit(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::StrokeMiterlimit)
        }
        AnimationKind::StrokeLinecap(track) => {
            sample_discrete(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::StrokeLinecap)
        }
        AnimationKind::StrokeLinejoin(track) => {
            sample_discrete(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::StrokeLinejoin)
        }
        AnimationKind::FillRule(track) => {
            sample_discrete(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::FillRule)
        }
        AnimationKind::Display(track) => {
            sample_discrete(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::Display)
        }
        AnimationKind::Visibility(track) => {
            sample_discrete_before_boundary(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::Visibility)
        }
        AnimationKind::Path(track) => sample_path(track, easing, timing_function, progress)
            .map(|(path, renderable)| SampledValue::Path(path, renderable)),
        AnimationKind::Geometry(_) => None,
        AnimationKind::StopOffset(track) => {
            sample_opacity(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::GradientGeometry)
        }
        AnimationKind::GradientGeometry(track) => {
            sample_scalar(track.track().keyframes(), easing, timing_function, progress)
                .map(SampledValue::GradientGeometry)
        }
        AnimationKind::ViewBox(track) => {
            sample_viewbox(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::ViewBox)
        }
        AnimationKind::ImageX(track)
        | AnimationKind::ImageY(track)
        | AnimationKind::ImageWidth(track)
        | AnimationKind::ImageHeight(track) => {
            sample_scalar(track.keyframes(), easing, timing_function, progress)
                .map(SampledValue::ImageGeometry)
        }
    }
}

#[cfg(test)]
include!("tests.rs");
