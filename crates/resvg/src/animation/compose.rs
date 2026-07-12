// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Animation sandwich composition.
//!
//! [`sample_overrides`] evaluates every animation on a node at one query time
//! and folds them into the per-attribute overrides the renderer applies over the
//! static value. SMIL animations form a priority sandwich ordered by their
//! contributing interval's begin time — a later-beginning animation sits higher
//! in the sandwich, with document order breaking ties. CSS animations composite
//! in their own list order over the SMIL result and are kept separate from the
//! SMIL begin-time rule.
//!
//! Within one attribute an `additive="replace"` animation overwrites the running
//! value (so the highest-priority contribution wins) while `additive="sum"`
//! composes onto it: matrices post-multiply, scalars and colors add. SMIL
//! `accumulate="sum"` adds `i × f(D)` for the current 0-based iteration `i`,
//! where `f(D)` is the value at the end of the simple duration; baked path tracks
//! use their precomputed accumulation delta instead.

use std::sync::Arc;

use svgtypes::Color;
use tiny_skia::{Path, PathBuilder, PathSegment, Transform};
use usvg::{
    Accumulate, Additive, Animation, AnimationKind, AnimationVisibility, Dur, Easing, FillRule,
    Interval, LineCap, LineJoin, NodeAnimation, NonZeroRect, SmilFill, SmilTiming, Timing,
    TimingFunction, TransformBox, TransformOrigin, TransformTrack,
};

use super::interpolate::{SampledValue, interpolate_track_with_timing};
use super::timing::{css_progress, smil_progress};

/// The sampled geometry of an animated `image` element.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageGeometry {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

/// The per-attribute animation overrides sampled at one time.
///
/// Each `Option` field is `Some` only when at least one animation contributes to
/// that attribute; the renderer falls back to the static value otherwise. The
/// stored values are the folded sandwich result, not deltas.
#[derive(Debug, Default)]
pub(crate) struct SampledOverrides {
    pub(crate) transform: Option<Transform>,
    pub(crate) opacity: Option<f32>,
    pub(crate) fill: Option<Color>,
    pub(crate) stroke: Option<Color>,
    pub(crate) stroke_width: Option<f32>,
    pub(crate) dashoffset: Option<f32>,
    pub(crate) dasharray: Option<Vec<f32>>,
    pub(crate) linecap: Option<LineCap>,
    pub(crate) linejoin: Option<LineJoin>,
    pub(crate) miterlimit: Option<f32>,
    pub(crate) fill_rule: Option<FillRule>,
    pub(crate) path: Option<(Arc<Path>, bool)>,
    pub(crate) paths: Vec<(Arc<Path>, bool)>,
    pub(crate) hidden: Option<bool>,
    pub(crate) gradient_overrides: Vec<(usize, SampledValue)>,
    pub(crate) view_box: Option<NonZeroRect>,
    pub(crate) image_geometry: Option<ImageGeometry>,
    pub(crate) css_transform: Option<(TransformOrigin, TransformBox)>,
}

/// One animation that contributes to the sandwich at the query time.
struct Contribution<'a> {
    animation: &'a Animation,
    progress: f32,
    iteration: u32,
    css: bool,
    begin: f32,
    order: usize,
}

/// Samples every animation on `node_anim` at time `t` and folds them into the
/// per-attribute sandwich.
pub(crate) fn sample_overrides(node_anim: &NodeAnimation, t: f32) -> SampledOverrides {
    let mut overrides = SampledOverrides::default();
    if node_anim.base_hidden() {
        overrides.hidden = Some(true);
    }
    let mut image = ImageState::new(node_anim);

    let mut contribution_iter = node_anim
        .animations()
        .iter()
        .enumerate()
        .filter(|(_, animation)| !animation.suppressed_by_important())
        .filter_map(|(order, animation)| build_contribution(animation, order, t));

    let Some(first) = contribution_iter.next() else {
        overrides.image_geometry = image.finish();
        return overrides;
    };

    let Some(second) = contribution_iter.next() else {
        fold(&mut overrides, &mut image, &first);
        overrides.image_geometry = image.finish();
        return overrides;
    };

    let mut contributions = vec![first, second];
    contributions.extend(contribution_iter);

    // SMIL sorts by interval begin (later wins) then document order; CSS sorts
    // after all SMIL contributions, in document order.
    contributions.sort_by(|a, b| {
        a.css
            .cmp(&b.css)
            .then(a.begin.total_cmp(&b.begin))
            .then(a.order.cmp(&b.order))
    });

    for contribution in &contributions {
        fold(&mut overrides, &mut image, contribution);
    }
    overrides.image_geometry = image.finish();
    overrides
}

/// Resolves an animation's progress and priority key at time `t`, or `None` when
/// it contributes nothing.
fn build_contribution(animation: &Animation, order: usize, t: f32) -> Option<Contribution<'_>> {
    match animation.timing() {
        Timing::Smil(smil) => {
            let progress = smil_progress(smil, t)?;
            let (begin, iteration) = smil_interval(smil, t)?;
            Some(Contribution {
                animation,
                progress,
                iteration,
                css: false,
                begin,
                order,
            })
        }
        Timing::Css(css) => {
            let progress = css_progress(css, t)?;
            Some(Contribution {
                animation,
                progress,
                iteration: 0,
                css: true,
                begin: 0.0,
                order,
            })
        }
    }
}

/// Locates the contributing SMIL interval and returns its begin and the 0-based
/// iteration index at `t`, mirroring [`smil_progress`]'s interval selection.
fn smil_interval(timing: &SmilTiming, t: f32) -> Option<(f32, u32)> {
    let mut most_recent: Option<&Interval> = None;
    for interval in timing.intervals() {
        if interval.begin() <= t {
            most_recent = Some(interval);
        }
        let active = match interval.end() {
            Some(end) => interval.begin() <= t && t < end,
            None => interval.begin() <= t,
        };
        if active {
            return Some((
                interval.begin(),
                active_iteration(interval.begin(), t, timing.dur()),
            ));
        }
    }
    let interval = most_recent?;
    match timing.fill() {
        SmilFill::Freeze => Some((interval.begin(), frozen_iteration(interval, timing.dur()))),
        SmilFill::Remove => None,
    }
}

/// The 0-based iteration index for an active interval.
fn active_iteration(begin: f32, t: f32, dur: &Dur) -> u32 {
    match *dur {
        Dur::Seconds(seconds) if seconds > 0.0 => ((t - begin) / seconds).floor().max(0.0) as u32,
        _ => 0,
    }
}

/// The 0-based iteration index held after a frozen interval ends.
///
/// A whole-number boundary freezes at the end of the last completed iteration,
/// matching [`smil_progress`]'s freeze-at-`1.0` rule.
fn frozen_iteration(interval: &Interval, dur: &Dur) -> u32 {
    let Some(end) = interval.end() else {
        return 0;
    };
    match *dur {
        Dur::Seconds(seconds) if seconds > 0.0 => {
            let raw = (end - interval.begin()) / seconds;
            let floored = raw.floor();
            if raw - floored <= f32::EPSILON && raw >= 1.0 {
                (floored - 1.0).max(0.0) as u32
            } else {
                floored.max(0.0) as u32
            }
        }
        _ => 0,
    }
}

/// Samples one contribution and folds it into the running overrides.
fn fold(overrides: &mut SampledOverrides, image: &mut ImageState, contribution: &Contribution) {
    let animation = contribution.animation;
    let timing_function = css_timing_function(animation);
    let Some(sampled) = interpolate_track_with_timing(
        animation.kind(),
        animation.easing(),
        timing_function,
        contribution.progress,
    ) else {
        return;
    };
    let sampled = match animation.accumulate() {
        Accumulate::Sum => accumulate(
            animation.kind(),
            animation.easing(),
            timing_function,
            sampled,
            contribution.iteration,
        ),
        Accumulate::None => sampled,
    };
    apply(
        overrides,
        image,
        animation.kind(),
        sampled,
        animation.additive(),
        contribution.order,
    );
}

/// Routes a sampled value into its override slot and folds by additivity.
fn apply(
    overrides: &mut SampledOverrides,
    image: &mut ImageState,
    kind: &AnimationKind,
    sampled: SampledValue,
    additive: Additive,
    order: usize,
) {
    match sampled {
        SampledValue::Transform(matrix) => {
            fold_transform(&mut overrides.transform, matrix, additive);
            if let AnimationKind::Transform(TransformTrack::Css { origin, box_, .. }) = kind {
                overrides.css_transform = Some((*origin, *box_));
            }
        }
        SampledValue::Motion(matrix) => {
            // Motion supplements the transform sandwich by post-multiplication.
            let base = overrides.transform.unwrap_or_else(Transform::identity);
            overrides.transform = Some(base.pre_concat(matrix));
        }
        SampledValue::Opacity(value) => match kind {
            AnimationKind::StopOpacity(_) => {
                push_gradient(overrides, order, SampledValue::Opacity(value));
            }
            _ => fold_scalar(&mut overrides.opacity, value, additive),
        },
        SampledValue::Color(color) => match kind {
            AnimationKind::Stroke(_) => fold_color(&mut overrides.stroke, color, additive),
            AnimationKind::StopColor(_) => {
                push_gradient(overrides, order, SampledValue::Color(color))
            }
            _ => fold_color(&mut overrides.fill, color, additive),
        },
        SampledValue::StrokeWidth(value) => {
            fold_scalar(&mut overrides.stroke_width, value, additive)
        }
        SampledValue::StrokeDashoffset(value) => {
            fold_scalar(&mut overrides.dashoffset, value, additive);
        }
        SampledValue::StrokeDasharray(values) => {
            fold_dasharray(&mut overrides.dasharray, values, additive);
        }
        SampledValue::StrokeMiterlimit(value) => {
            fold_scalar(&mut overrides.miterlimit, value, additive);
        }
        SampledValue::StrokeLinecap(cap) => overrides.linecap = Some(cap),
        SampledValue::StrokeLinejoin(join) => overrides.linejoin = Some(join),
        SampledValue::FillRule(rule) => overrides.fill_rule = Some(rule),
        SampledValue::Display(shown) => overrides.hidden = Some(!shown),
        SampledValue::Visibility(visibility) => {
            overrides.hidden = Some(!matches!(visibility, AnimationVisibility::Visible));
        }
        SampledValue::Path(path, renderable) => {
            if matches!(kind, AnimationKind::Path(track) if track.replaces_geometry()) {
                overrides.paths.clear();
            }
            overrides.path = Some((path.clone(), renderable));
            overrides.paths.push((path, renderable));
        }
        SampledValue::GradientGeometry(value) => {
            push_gradient(overrides, order, SampledValue::GradientGeometry(value));
        }
        SampledValue::ViewBox(rect) => overrides.view_box = Some(rect),
        SampledValue::ImageGeometry(value) => {
            if let Some(index) = image_component(kind) {
                image.set(index, value, additive);
            }
        }
    }
}

fn css_timing_function(animation: &Animation) -> Option<&TimingFunction> {
    match animation.timing() {
        Timing::Css(timing) => Some(timing.timing_function()),
        Timing::Smil(_) => None,
    }
}

/// Records a gradient stop or geometry override keyed by its arrival order.
fn push_gradient(overrides: &mut SampledOverrides, index: usize, value: SampledValue) {
    overrides.gradient_overrides.push((index, value));
}

/// Maps an image-geometry kind to its quad component index (`x`, `y`, `w`, `h`).
fn image_component(kind: &AnimationKind) -> Option<usize> {
    match kind {
        AnimationKind::ImageX(_) => Some(0),
        AnimationKind::ImageY(_) => Some(1),
        AnimationKind::ImageWidth(_) => Some(2),
        AnimationKind::ImageHeight(_) => Some(3),
        _ => None,
    }
}

/// Folds a scalar: `Replace` overwrites, `Sum` adds onto the running value.
fn fold_scalar(slot: &mut Option<f32>, value: f32, additive: Additive) {
    *slot = Some(match (*slot, additive) {
        (Some(current), Additive::Sum) => current + value,
        _ => value,
    });
}

/// Folds a matrix: `Replace` overwrites, `Sum` post-multiplies (`sandwich × m`).
fn fold_transform(slot: &mut Option<Transform>, matrix: Transform, additive: Additive) {
    *slot = Some(match (*slot, additive) {
        (Some(current), Additive::Sum) => current.pre_concat(matrix),
        _ => matrix,
    });
}

/// Folds a color: `Replace` overwrites, `Sum` adds each channel with saturation.
fn fold_color(slot: &mut Option<Color>, color: Color, additive: Additive) {
    *slot = Some(match (*slot, additive) {
        (Some(current), Additive::Sum) => add_color(current, color, 1),
        _ => color,
    });
}

/// Folds a dash array: `Replace` overwrites, `Sum` adds element-wise.
fn fold_dasharray(slot: &mut Option<Vec<f32>>, values: Vec<f32>, additive: Additive) {
    *slot = Some(match (slot.take(), additive) {
        (Some(current), Additive::Sum) => {
            let len = current.len().min(values.len());
            (0..len).map(|i| current[i] + values[i]).collect()
        }
        _ => values,
    });
}

/// Applies `accumulate="sum"` to a sampled value for iteration `iteration`.
fn accumulate(
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
fn add_color(base: Color, delta: Color, times: u32) -> Color {
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

/// The running quad for `image` geometry, seeded from the static carrier so an
/// animation of one component keeps the others at their static value.
struct ImageState {
    quad: (f32, f32, f32, f32),
    touched: bool,
    available: bool,
}

impl ImageState {
    fn new(node_anim: &NodeAnimation) -> Self {
        match node_anim.image() {
            Some(image) => {
                let (x, y, w, h) = image.static_quad();
                Self {
                    quad: (x, y, w, h),
                    touched: false,
                    available: true,
                }
            }
            None => Self {
                quad: (0.0, 0.0, 0.0, 0.0),
                touched: false,
                available: false,
            },
        }
    }

    fn set(&mut self, index: usize, value: f32, additive: Additive) {
        let slot = match index {
            0 => &mut self.quad.0,
            1 => &mut self.quad.1,
            2 => &mut self.quad.2,
            _ => &mut self.quad.3,
        };
        *slot = match additive {
            Additive::Sum => *slot + value,
            Additive::Replace => value,
        };
        self.touched = true;
    }

    fn finish(self) -> Option<ImageGeometry> {
        (self.available && self.touched).then(|| ImageGeometry {
            x: self.quad.0,
            y: self.quad.1,
            w: self.quad.2,
            h: self.quad.3,
        })
    }
}

fn warn_accumulate_ignored() {
    log::warn!("Unsupported accumulate value; ignoring.");
}

#[cfg(test)]
mod tests {
    use super::*;

    use usvg::{
        AnimationSource, Begin, CalcMode, CssFillMode, CssTiming, Direction, Iterations, Keyframe,
        MotionRotate, MotionTrack, NormalizedF32, PathKeyframe, PathTrack, PlayState, Restart,
        TimingFunction, Track, TransformKind, TransformTrack,
    };

    fn n(value: f32) -> NormalizedF32 {
        NormalizedF32::new_clamped(value)
    }

    fn linear() -> Easing {
        Easing::new(CalcMode::Linear, None, None)
    }

    fn discrete() -> Easing {
        Easing::new(CalcMode::Discrete, None, None)
    }

    fn interval(begin: f32, end: Option<f32>) -> Interval {
        Interval::new(begin, end)
    }

    fn smil(dur: f32, intervals: Vec<Interval>, fill: SmilFill) -> Timing {
        Timing::Smil(SmilTiming::new(
            vec![Begin::Offset(0.0)],
            Dur::Seconds(dur),
            vec![],
            None,
            None,
            fill,
            Restart::Always,
            intervals,
        ))
    }

    fn animation(
        kind: AnimationKind,
        timing: Timing,
        easing: Easing,
        additive: Additive,
        accumulate: Accumulate,
    ) -> Arc<Animation> {
        Arc::new(Animation::new(
            kind,
            timing,
            easing,
            additive,
            accumulate,
            AnimationSource::Smil,
            false,
        ))
    }

    fn node(animations: Vec<Arc<Animation>>) -> NodeAnimation {
        NodeAnimation::new(animations, false, None, None, None, None)
    }

    fn width_track(values: &[f32]) -> Track<f32> {
        Track::new(
            values
                .iter()
                .enumerate()
                .map(|(index, &value)| {
                    let offset = if values.len() <= 1 {
                        0.0
                    } else {
                        index as f32 / (values.len() - 1) as f32
                    };
                    Keyframe::new(n(offset), value, None)
                })
                .collect(),
        )
    }

    fn width_animation(
        values: &[f32],
        timing: Timing,
        additive: Additive,
        accumulate: Accumulate,
    ) -> Arc<Animation> {
        animation(
            AnimationKind::StrokeWidth(width_track(values)),
            timing,
            linear(),
            additive,
            accumulate,
        )
    }

    fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Arc<Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(x, y);
        builder.line_to(x + w, y);
        builder.line_to(x + w, y + h);
        builder.line_to(x, y + h);
        builder.close();
        Arc::new(builder.finish().unwrap())
    }

    fn delta_path(points: &[(f32, f32)]) -> Arc<Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            builder.line_to(x, y);
        }
        builder.close();
        Arc::new(builder.finish().unwrap())
    }

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "expected {b}, got {a}");
    }

    fn approx_transform(a: Transform, b: Transform) {
        approx(a.sx, b.sx);
        approx(a.ky, b.ky);
        approx(a.kx, b.kx);
        approx(a.sy, b.sy);
        approx(a.tx, b.tx);
        approx(a.ty, b.ty);
    }

    #[test]
    fn additive_rotate_over_base_transform() {
        // A Replace translate establishes the base; an additive rotate composes
        // onto it by post-multiplication.
        let base = animation(
            AnimationKind::Transform(TransformTrack::Smil {
                kind: TransformKind::Translate,
                keyframes: vec![Keyframe::new(n(0.0), vec![30.0, 0.0], None)],
            }),
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            linear(),
            Additive::Replace,
            Accumulate::None,
        );
        let rotate = animation(
            AnimationKind::Transform(TransformTrack::Smil {
                kind: TransformKind::Rotate,
                keyframes: vec![Keyframe::new(n(0.0), vec![90.0, 0.0, 0.0], None)],
            }),
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            linear(),
            Additive::Sum,
            Accumulate::None,
        );

        let overrides = sample_overrides(&node(vec![base, rotate]), 0.5);
        let expected =
            Transform::from_translate(30.0, 0.0).pre_concat(Transform::from_rotate(90.0));
        approx_transform(overrides.transform.unwrap(), expected);
    }

    #[test]
    fn priority_by_begin_beats_document_order() {
        // Document order 0 begins later (at 2s) than document order 1 (at 0s).
        // At t=3 the later-beginning animation wins despite coming first.
        let late = width_animation(
            &[20.0],
            smil(10.0, vec![interval(2.0, Some(12.0))], SmilFill::Freeze),
            Additive::Replace,
            Accumulate::None,
        );
        let early = width_animation(
            &[10.0],
            smil(10.0, vec![interval(0.0, Some(10.0))], SmilFill::Freeze),
            Additive::Replace,
            Accumulate::None,
        );

        let overrides = sample_overrides(&node(vec![late, early]), 3.0);
        approx(overrides.stroke_width.unwrap(), 20.0);
    }

    #[test]
    fn document_order_breaks_equal_begin_tie() {
        // Equal begins: the later document-order animation wins the tie.
        let first = width_animation(
            &[10.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            Additive::Replace,
            Accumulate::None,
        );
        let second = width_animation(
            &[20.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            Additive::Replace,
            Accumulate::None,
        );

        let overrides = sample_overrides(&node(vec![first, second]), 0.5);
        approx(overrides.stroke_width.unwrap(), 20.0);
    }

    #[test]
    fn accumulate_scalar_adds_end_value_per_iteration() {
        // from=2 to=4 dur=1 repeatCount=2 accumulate=sum: iteration 1 spans 6->8.
        let anim = width_animation(
            &[2.0, 4.0],
            smil(1.0, vec![interval(0.0, Some(2.0))], SmilFill::Freeze),
            Additive::Replace,
            Accumulate::Sum,
        );
        let node = node(vec![anim]);

        approx(sample_overrides(&node, 1.0).stroke_width.unwrap(), 6.0);
        approx(sample_overrides(&node, 1.5).stroke_width.unwrap(), 7.0);
        approx(sample_overrides(&node, 2.0).stroke_width.unwrap(), 8.0);
    }

    #[test]
    fn accumulate_baked_path_uses_delta() {
        // Iteration 1 at progress 0 offsets the first keyframe by one delta,
        // which equals the second keyframe's geometry.
        let first = rect_path(0.0, 0.0, 10.0, 10.0);
        let second = rect_path(20.0, 0.0, 10.0, 10.0);
        let delta = delta_path(&[(20.0, 0.0), (20.0, 0.0), (20.0, 0.0), (20.0, 0.0)]);
        let track = PathTrack::new(
            vec![
                PathKeyframe::new(n(0.0), first, true, None),
                PathKeyframe::new(n(1.0), second.clone(), true, None),
            ],
            Some(delta),
        );
        let anim = animation(
            AnimationKind::Path(track),
            smil(1.0, vec![interval(0.0, Some(2.0))], SmilFill::Freeze),
            linear(),
            Additive::Replace,
            Accumulate::Sum,
        );

        let overrides = sample_overrides(&node(vec![anim]), 1.0);
        let (path, renderable) = overrides.path.unwrap();
        assert!(renderable);
        let got = path.points();
        let want = second.points();
        assert_eq!(got.len(), want.len());
        for (a, b) in got.iter().zip(want.iter()) {
            approx(a.x, b.x);
            approx(a.y, b.y);
        }
    }

    #[test]
    fn accumulate_ignored_on_discrete_fill_rule() {
        // A discrete fill-rule track cannot accumulate: the sampled value passes
        // through unchanged (and the code emits the ignore warning).
        let track = Track::new(vec![
            Keyframe::new(n(0.0), FillRule::EvenOdd, None),
            Keyframe::new(n(1.0), FillRule::NonZero, None),
        ]);
        let anim = animation(
            AnimationKind::FillRule(track),
            smil(1.0, vec![interval(0.0, Some(2.0))], SmilFill::Freeze),
            discrete(),
            Additive::Replace,
            Accumulate::Sum,
        );

        // t=1.25 is iteration 1 at local progress 0.25 -> discrete index 0.
        let overrides = sample_overrides(&node(vec![anim]), 1.25);
        assert!(matches!(overrides.fill_rule, Some(FillRule::EvenOdd)));
    }

    #[test]
    fn by_animation_composes_over_static_base() {
        // A Replace base of 100 with an additive [0, by] Sum of 10: at half-time
        // the by-animation adds half its range (5) onto the base -> 105.
        let base = width_animation(
            &[100.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            Additive::Replace,
            Accumulate::None,
        );
        let by = width_animation(
            &[0.0, 10.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            Additive::Sum,
            Accumulate::None,
        );

        let overrides = sample_overrides(&node(vec![base, by]), 0.5);
        approx(overrides.stroke_width.unwrap(), 105.0);
    }

    #[test]
    fn motion_matrix_pins_translate_and_rotation() {
        // A diagonal line: at half arc-length the point is (50,50) and the
        // tangent is 45 degrees, so the local transform is T(50,50) * R(45).
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 100.0);
        let path = Arc::new(builder.finish().unwrap());
        let anim = animation(
            AnimationKind::Motion(MotionTrack::new(path, None, MotionRotate::Auto)),
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            linear(),
            Additive::Replace,
            Accumulate::None,
        );

        let overrides = sample_overrides(&node(vec![anim]), 0.5);
        let expected =
            Transform::from_translate(50.0, 50.0).pre_concat(Transform::from_rotate(45.0));
        approx_transform(overrides.transform.unwrap(), expected);
    }

    #[test]
    fn inactive_remove_contributes_nothing() {
        // t=2 is past the [0,1) interval with fill=remove: nothing contributes.
        let anim = width_animation(
            &[50.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Remove),
            Additive::Replace,
            Accumulate::None,
        );

        let overrides = sample_overrides(&node(vec![anim]), 2.0);
        assert!(overrides.stroke_width.is_none());
    }

    #[test]
    fn suppressed_by_important_is_skipped() {
        // An important static declaration suppresses its animation entirely.
        let suppressed = Arc::new(Animation::new(
            AnimationKind::StrokeWidth(width_track(&[50.0])),
            smil(1.0, vec![interval(0.0, Some(1.0))], SmilFill::Freeze),
            linear(),
            Additive::Replace,
            Accumulate::None,
            AnimationSource::Smil,
            true,
        ));

        let overrides = sample_overrides(&node(vec![suppressed]), 0.5);
        assert!(overrides.stroke_width.is_none());
    }

    #[test]
    fn css_timing_function_shapes_the_sampled_progress() {
        let animation = width_animation(
            &[0.0, 100.0],
            Timing::Css(CssTiming::new(
                2.0,
                0.0,
                Iterations::Count(1.0),
                Direction::Normal,
                CssFillMode::Both,
                TimingFunction::CubicBezier(0.42, 0.0, 1.0, 1.0),
                PlayState::Running,
            )),
            Additive::Replace,
            Accumulate::None,
        );

        let sampled = sample_overrides(&node(vec![animation]), 1.0)
            .stroke_width
            .unwrap();
        assert!(sampled < 45.0, "expected eased progress, got {sampled}");
    }
}
