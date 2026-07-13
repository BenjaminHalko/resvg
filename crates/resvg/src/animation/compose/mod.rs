// Copyright 2026 the Resvg Authors
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

mod accumulate;
mod apply;
mod sandwich;

pub(crate) use sandwich::sample_overrides;

use std::sync::Arc;

use svgtypes::Color;
use tiny_skia::{Path, Transform};
use usvg::{FillRule, LineCap, LineJoin, NonZeroRect, TransformBox, TransformOrigin};

use super::interpolate::SampledValue;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tiny_skia::{Path, PathBuilder, Transform};
    use usvg::{
        Accumulate, Additive, Animation, AnimationKind, AnimationSource, CalcMode, Direction,
        Easing, FillRule, Interval, Keyframe, MotionRotate, MotionTrack, NodeAnimation,
        NormalizedF32, PathKeyframe, PathTrack, TimedInterval, Timing, TimingFunction, Track,
        TransformKind, TransformTrack,
    };

    use super::*;

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

    fn smil(dur: f32, intervals: Vec<Interval>, freeze: bool) -> Timing {
        let one_loop_end = intervals
            .iter()
            .map(Interval::begin)
            .reduce(f32::min)
            .map(|begin| begin + dur);
        let intervals = intervals
            .into_iter()
            .map(|interval| {
                let held = freeze.then(|| {
                    let end = interval.end().unwrap_or(interval.begin());
                    let raw = (end - interval.begin()) / dur;
                    let fraction = raw - raw.floor();
                    if fraction <= f32::EPSILON && raw >= 1.0 {
                        1.0
                    } else {
                        fraction
                    }
                });
                TimedInterval::new(interval, held)
            })
            .collect();
        Timing::new(intervals, Some(dur), Direction::Normal, None, one_loop_end)
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
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
            linear(),
            Additive::Replace,
            Accumulate::None,
        );
        let rotate = animation(
            AnimationKind::Transform(TransformTrack::Smil {
                kind: TransformKind::Rotate,
                keyframes: vec![Keyframe::new(n(0.0), vec![90.0, 0.0, 0.0], None)],
            }),
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
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
            smil(10.0, vec![interval(2.0, Some(12.0))], true),
            Additive::Replace,
            Accumulate::None,
        );
        let early = width_animation(
            &[10.0],
            smil(10.0, vec![interval(0.0, Some(10.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
            Additive::Replace,
            Accumulate::None,
        );
        let second = width_animation(
            &[20.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(2.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(2.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(2.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
            Additive::Replace,
            Accumulate::None,
        );
        let by = width_animation(
            &[0.0, 10.0],
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
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
            smil(1.0, vec![interval(0.0, Some(1.0))], false),
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
            smil(1.0, vec![interval(0.0, Some(1.0))], true),
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
        let animation = animation(
            AnimationKind::StrokeWidth(width_track(&[0.0, 100.0])),
            Timing::new(
                vec![TimedInterval::new(
                    Interval::new_relative(0.0, 2.0),
                    Some(1.0),
                )],
                Some(2.0),
                Direction::Normal,
                Some(0.0),
                Some(2.0),
            ),
            linear().with_timing_function(TimingFunction::CubicBezier(0.42, 0.0, 1.0, 1.0)),
            Additive::Replace,
            Accumulate::None,
        );

        let sampled = sample_overrides(&node(vec![animation]), 1.0)
            .stroke_width
            .unwrap();
        assert!(sampled < 45.0, "expected eased progress, got {sampled}");
    }
}
