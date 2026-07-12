// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::too_many_arguments)]

use std::sync::Arc;

use tiny_skia_path::Path;

#[cfg(test)]
use tiny_skia_path::Point;

use crate::NormalizedF32;
use crate::parser::shapes::{animated_ellipse_path, animated_rect_path};
#[cfg(test)]
use crate::parser::shapes::{circle_path, polyline_path, rect_path};
use crate::parser::svgtree::EId;
use crate::tree::animation::{
    Accumulate, AnimationKind, CalcMode, PathKeyframe, PathTrack, TimingFunction,
};

use super::GeometryBake;
use super::accumulate::{bake_accumulation, verbs_match};
use super::keyframes::{
    Baked, RawKeyframe, bake_path_data, bake_points, warn_invalid_geometry_value,
};
use super::shape::{ShapeGeometry, build_shape_path, is_shape_renderable};

/// Bakes a geometry attribute animation into an [`AnimationKind::Path`] track.
///
/// `keyframe_values` carries the resolved scalar per keyframe for a shape
/// scalar attribute; `d_keyframes`/`points_keyframes` carry the raw keyframe
/// strings for the `d` and `points` attributes. Returns `None` when nothing
/// renderable could be baked.
pub(crate) fn bake_geometry_animation(
    element_tag: EId,
    attribute_name: &str,
    base: ShapeGeometry,
    keyframe_values: &[f32],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
    calc_mode: CalcMode,
    accumulate: Accumulate,
    d_keyframes: Option<&[&str]>,
    points_keyframes: Option<&[&str]>,
) -> Option<GeometryBake> {
    bake_geometry_animation_inner(
        element_tag,
        attribute_name,
        base,
        keyframe_values,
        key_offsets,
        key_timing_fns,
        calc_mode,
        accumulate,
        d_keyframes,
        points_keyframes,
        false,
    )
}

pub(crate) fn bake_geometry_animation_with_sum_base(
    element_tag: EId,
    attribute_name: &str,
    base: ShapeGeometry,
    keyframe_values: &[f32],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
    calc_mode: CalcMode,
    accumulate: Accumulate,
    d_keyframes: Option<&[&str]>,
    points_keyframes: Option<&[&str]>,
) -> Option<GeometryBake> {
    bake_geometry_animation_inner(
        element_tag,
        attribute_name,
        base,
        keyframe_values,
        key_offsets,
        key_timing_fns,
        calc_mode,
        accumulate,
        d_keyframes,
        points_keyframes,
        true,
    )
}

fn bake_geometry_animation_inner(
    element_tag: EId,
    attribute_name: &str,
    base: ShapeGeometry,
    keyframe_values: &[f32],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
    calc_mode: CalcMode,
    accumulate: Accumulate,
    d_keyframes: Option<&[&str]>,
    points_keyframes: Option<&[&str]>,
    accumulate_from_base: bool,
) -> Option<GeometryBake> {
    let (
        Baked {
            keyframes,
            multi_param,
        },
        accumulation_origin,
    ) = match attribute_name {
        "d" => (
            bake_path_data(d_keyframes?, key_offsets, key_timing_fns)?,
            None,
        ),
        "points" => (
            bake_points(element_tag, points_keyframes?, key_offsets, key_timing_fns)?,
            None,
        ),
        _ => (
            bake_scalar(
                element_tag,
                attribute_name,
                base,
                keyframe_values,
                key_offsets,
                key_timing_fns,
            )?,
            if accumulate_from_base {
                build_shape_path(element_tag, &base)
            } else {
                base.with_attribute(attribute_name, 0.0)
                    .and_then(|geometry| build_shape_path(element_tag, &geometry))
            },
        ),
    };

    if keyframes.is_empty() {
        return None;
    }

    // Point-wise interpolation requires an identical verb sequence across all
    // keyframes; otherwise fall back to discrete stepping.
    let interpolable = verbs_match(&keyframes);
    let calc_mode = if interpolable {
        calc_mode
    } else {
        log::warn!("Animation values are not interpolable; using discrete interpolation.");
        CalcMode::Discrete
    };

    let accumulation_delta = bake_accumulation(
        &keyframes,
        accumulate,
        multi_param,
        interpolable,
        accumulation_origin.as_ref(),
    );

    let path_keyframes = keyframes
        .into_iter()
        .map(|k| PathKeyframe::new(k.offset, Arc::new(k.path), k.renderable, k.timing_function))
        .collect();

    let track = if matches!(attribute_name, "d" | "points") {
        PathTrack::new_replacing_geometry(path_keyframes, accumulation_delta)
    } else {
        PathTrack::new(path_keyframes, accumulation_delta)
    };

    Some(GeometryBake {
        kind: AnimationKind::Path(track),
        calc_mode,
    })
}

/// Bakes a shape scalar attribute (`width`, `r`, `cx`, ...) per keyframe.
fn bake_scalar(
    element_tag: EId,
    attribute_name: &str,
    base: ShapeGeometry,
    keyframe_values: &[f32],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
) -> Option<Baked> {
    let non_negative = is_non_negative_attribute(attribute_name);

    let mut keyframes = Vec::new();
    for (i, &value) in keyframe_values.iter().enumerate() {
        let offset = *key_offsets.get(i)?;
        let timing_function = key_timing_fns.get(i).copied().flatten();

        // A clean zero bakes to a non-renderable degenerate snapshot; a negative
        // or non-finite value is dropped.
        if !value.is_finite() || (non_negative && value < 0.0) {
            warn_invalid_geometry_value(value);
            continue;
        }

        let geometry = base.with_attribute(attribute_name, value)?;
        let path = build_animated_shape_path(element_tag, attribute_name, geometry)?;
        let renderable = is_shape_renderable(element_tag, &geometry);

        keyframes.push(RawKeyframe {
            offset,
            path,
            renderable,
            timing_function,
        });
    }

    Some(Baked {
        keyframes,
        multi_param: false,
    })
}

fn build_animated_shape_path(
    element_tag: EId,
    attribute_name: &str,
    geometry: ShapeGeometry,
) -> Option<Path> {
    if element_tag == EId::Rect && matches!(attribute_name, "rx" | "ry") {
        return animated_rect_path(
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            geometry.rx,
            geometry.ry,
        );
    }
    if element_tag == EId::Circle && attribute_name == "r" {
        return animated_ellipse_path(geometry.cx, geometry.cy, geometry.r, geometry.r);
    }
    if element_tag == EId::Ellipse && matches!(attribute_name, "rx" | "ry") {
        return animated_ellipse_path(geometry.cx, geometry.cy, geometry.rx, geometry.ry);
    }
    build_shape_path(element_tag, &geometry)
}

/// Reports whether an attribute must be non-negative to bake a real shape.
fn is_non_negative_attribute(attribute_name: &str) -> bool {
    matches!(
        attribute_name,
        "width" | "height" | "r" | "rx" | "ry" | "fr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: f32) -> NormalizedF32 {
        NormalizedF32::new_clamped(v)
    }

    /// Asserts that the point-wise midpoint of `a` and `b` equals `expected`.
    fn assert_midpoint_matches(a: &Path, b: &Path, expected: &Path) {
        let ap = a.points();
        let bp = b.points();
        let ep = expected.points();
        assert_eq!(ap.len(), ep.len(), "keyframe/expected point count mismatch");
        assert_eq!(bp.len(), ep.len(), "keyframe/expected point count mismatch");
        for i in 0..ep.len() {
            let mx = (ap[i].x + bp[i].x) / 2.0;
            let my = (ap[i].y + bp[i].y) / 2.0;
            assert!((mx - ep[i].x).abs() < 1e-4, "x[{i}]: {mx} vs {}", ep[i].x);
            assert!((my - ep[i].y).abs() < 1e-4, "y[{i}]: {my} vs {}", ep[i].y);
        }
    }

    fn expect_track(bake: &GeometryBake) -> &PathTrack {
        match &bake.kind {
            AnimationKind::Path(track) => track,
            other => panic!("expected a path track, got {other:?}"),
        }
    }

    #[test]
    fn move_only_path_keyframe_is_preserved_as_nonrenderable() {
        let baked = bake_path_data(&["M-338,462"], &[n(0.0)], &[None]).unwrap();

        assert_eq!(baked.keyframes.len(), 1);
        assert!(!baked.keyframes[0].renderable);
    }

    #[test]
    fn circle_radius_midpoint_is_exact() {
        let base = ShapeGeometry {
            cx: 100.0,
            cy: 100.0,
            r: 40.0,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Circle,
            "r",
            base,
            &[40.0, 60.0],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        assert_eq!(track.keyframes().len(), 2);
        assert!(matches!(bake.calc_mode, CalcMode::Linear));

        let kf0 = track.keyframes()[0].path();
        let kf1 = track.keyframes()[1].path();
        assert_eq!(kf0.verbs(), kf1.verbs());

        let expected = circle_path(100.0, 100.0, 50.0).unwrap();
        assert_midpoint_matches(kf0, kf1, &expected);
    }

    #[test]
    fn rect_grows_from_zero_width() {
        let base = ShapeGeometry {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 80.0,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Rect,
            "width",
            base,
            &[0.0, 100.0],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        assert_eq!(track.keyframes().len(), 2);
        assert!(matches!(bake.calc_mode, CalcMode::Linear));

        let kf0 = &track.keyframes()[0];
        let kf1 = &track.keyframes()[1];
        // The zero-width snapshot is degenerate but shares the verb sequence.
        assert!(!kf0.renderable());
        assert!(kf1.renderable());
        assert_eq!(kf0.path().verbs(), kf1.path().verbs());

        let expected = rect_path(0.0, 0.0, 50.0, 80.0, 0.0, 0.0).unwrap();
        assert_midpoint_matches(kf0.path(), kf1.path(), &expected);
    }

    #[test]
    fn points_track_bakes() {
        let bake = bake_geometry_animation(
            EId::Polygon,
            "points",
            ShapeGeometry::default(),
            &[],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            Some(&["0,0 10,0 10,10", "0,0 20,0 20,20"]),
        )
        .unwrap();

        let track = expect_track(&bake);
        assert_eq!(track.keyframes().len(), 2);
        assert!(matches!(bake.calc_mode, CalcMode::Linear));

        let kf0 = track.keyframes()[0].path();
        let kf1 = track.keyframes()[1].path();
        assert_eq!(kf0.verbs(), kf1.verbs());

        // A polygon is closed; the midpoint tracks the moving vertices.
        let expected = polyline_path(
            &[
                Point::from_xy(0.0, 0.0),
                Point::from_xy(15.0, 0.0),
                Point::from_xy(15.0, 15.0),
            ],
            true,
        )
        .unwrap();
        assert_midpoint_matches(kf0, kf1, &expected);
    }

    #[test]
    fn d_verb_mismatch_forces_discrete() {
        let bake = bake_geometry_animation(
            EId::Path,
            "d",
            ShapeGeometry::default(),
            &[],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            Some(&["M0 0 L10 10", "M0 0 L10 10 L20 0"]),
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        assert_eq!(track.keyframes().len(), 2);
        assert!(matches!(bake.calc_mode, CalcMode::Discrete));
    }

    #[test]
    fn points_count_mismatch_forces_discrete() {
        let bake = bake_geometry_animation(
            EId::Polyline,
            "points",
            ShapeGeometry::default(),
            &[],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            Some(&["0,0 10,0", "0,0 10,0 20,20"]),
        )
        .unwrap();

        let track = expect_track(&bake);
        assert_eq!(track.keyframes().len(), 2);
        assert!(matches!(bake.calc_mode, CalcMode::Discrete));
    }

    #[test]
    fn negative_value_is_dropped() {
        let base = ShapeGeometry {
            cx: 50.0,
            cy: 50.0,
            r: 10.0,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Circle,
            "r",
            base,
            &[10.0, -5.0, 20.0],
            &[n(0.0), n(0.5), n(1.0)],
            &[None, None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        // The negative middle keyframe is dropped; the two valid ones remain.
        assert_eq!(track.keyframes().len(), 2);
        assert!(track.keyframes().iter().all(|k| k.renderable()));
    }

    #[test]
    fn zero_value_bakes_degenerate_keyframe() {
        let base = ShapeGeometry {
            cx: 50.0,
            cy: 50.0,
            r: 0.0,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Circle,
            "r",
            base,
            &[0.0],
            &[n(0.0)],
            &[None],
            CalcMode::Discrete,
            Accumulate::None,
            None,
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        assert_eq!(track.keyframes().len(), 1);
        // A clean zero bakes to a non-renderable keyframe rather than being dropped.
        assert!(!track.keyframes()[0].renderable());
    }

    #[test]
    fn accumulate_sum_bakes_delta() {
        let base = ShapeGeometry {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Rect,
            "width",
            base,
            &[10.0, 30.0],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::Sum,
            None,
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        let delta = track
            .accumulation_delta()
            .expect("accumulation delta baked");
        // width 10 -> 30: each repeat accumulates the final value, so the
        // right edge shifts by 30 while the left edge stays fixed.
        let points = delta.points();
        assert!(points[0].x.abs() < 1e-4);
        assert!((points[1].x - 30.0).abs() < 1e-4);
        assert!((points[2].x - 30.0).abs() < 1e-4);
        assert!(points[3].x.abs() < 1e-4);
    }

    #[test]
    fn rect_radius_animation_keeps_cubic_keyframes() {
        let base = ShapeGeometry {
            width: 160.0,
            height: 160.0,
            ry_is_implicit: true,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Rect,
            "rx",
            base,
            &[0.0, 120.0],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            None,
        )
        .unwrap();
        assert!(matches!(bake.calc_mode, CalcMode::Linear));
    }

    #[test]
    fn accumulate_on_d_is_ignored() {
        let bake = bake_geometry_animation(
            EId::Path,
            "d",
            ShapeGeometry::default(),
            &[],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::Sum,
            Some(&["M0 0 L10 10", "M0 0 L20 20"]),
            None,
        )
        .unwrap();

        let track = expect_track(&bake);
        assert!(track.accumulation_delta().is_none());
    }

    #[test]
    fn all_invalid_values_bake_nothing() {
        let base = ShapeGeometry {
            cx: 50.0,
            cy: 50.0,
            r: 10.0,
            ..ShapeGeometry::default()
        };
        let bake = bake_geometry_animation(
            EId::Circle,
            "r",
            base,
            &[-1.0, -2.0],
            &[n(0.0), n(1.0)],
            &[None, None],
            CalcMode::Linear,
            Accumulate::None,
            None,
            None,
        );
        assert!(bake.is_none());
    }
}
