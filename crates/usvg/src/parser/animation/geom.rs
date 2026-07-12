// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Baking of geometry attribute animations into path-data keyframe tracks.
//!
//! Shape geometry attributes (`width`, `r`, `cx`, ...) and the `d`/`points`
//! attributes are baked into a [`PathTrack`] at parse time by substituting each
//! keyframe value into the corresponding shape builder. A keyframe sequence is
//! point-wise interpolable only when every snapshot shares one verb sequence;
//! otherwise the track falls back to discrete stepping.

use std::sync::Arc;

use tiny_skia_path::{Path, PathBuilder, PathSegment, Point};

use crate::parser::shapes::{circle_path, ellipse_path, line_path, polyline_path, rect_path};
use crate::parser::svgtree::EId;
use crate::tree::animation::{
    Accumulate, AnimationKind, CalcMode, PathKeyframe, PathTrack, TimingFunction,
};
use crate::{IsValidLength, NormalizedF32};

/// The result of baking a geometry animation into a path track.
pub(crate) struct GeometryBake {
    /// The baked path track.
    pub(crate) kind: AnimationKind,
    /// The calculation mode, forced to `Discrete` when keyframes are not
    /// point-wise interpolable.
    pub(crate) calc_mode: CalcMode,
}

/// The resolved static geometry of a shape.
///
/// The animated attribute is overridden per keyframe; the remaining fields
/// supply the shape's other parameters.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ShapeGeometry {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) rx: f32,
    pub(crate) ry: f32,
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    pub(crate) r: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
    pub(crate) x2: f32,
    pub(crate) y2: f32,
}

impl ShapeGeometry {
    /// Returns a copy with `attribute` set to `value`, or `None` when the
    /// attribute is not a shape geometry scalar.
    fn with_attribute(mut self, attribute: &str, value: f32) -> Option<Self> {
        match attribute {
            "x" => self.x = value,
            "y" => self.y = value,
            "width" => self.width = value,
            "height" => self.height = value,
            "rx" => self.rx = value,
            "ry" => self.ry = value,
            "cx" => self.cx = value,
            "cy" => self.cy = value,
            "r" => self.r = value,
            "x1" => self.x1 = value,
            "y1" => self.y1 = value,
            "x2" => self.x2 = value,
            "y2" => self.y2 = value,
            _ => return None,
        }
        Some(self)
    }
}

/// A baked keyframe before its path is shared behind an `Arc`.
struct RawKeyframe {
    offset: NormalizedF32,
    path: Path,
    renderable: bool,
    timing_function: Option<TimingFunction>,
}

/// A baked keyframe sequence.
struct Baked {
    keyframes: Vec<RawKeyframe>,
    /// Whether the value interpolates multiple parameters (`d`/`points`), which
    /// cannot accumulate.
    multi_param: bool,
}

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
            base.with_attribute(attribute_name, 0.0)
                .and_then(|geometry| build_shape_path(element_tag, &geometry)),
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
        warn_not_interpolable();
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

    Some(GeometryBake {
        kind: AnimationKind::Path(PathTrack::new(path_keyframes, accumulation_delta)),
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
        let path = build_shape_path(element_tag, &geometry)?;
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

/// Bakes each `d` keyframe by parsing it as absolute path data.
fn bake_path_data(
    d_keyframes: &[&str],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
) -> Option<Baked> {
    let mut keyframes = Vec::new();
    for (i, &raw) in d_keyframes.iter().enumerate() {
        let offset = *key_offsets.get(i)?;
        let timing_function = key_timing_fns.get(i).copied().flatten();

        let Some(path) = parse_path_data(raw) else {
            warn_invalid_geometry_value(raw);
            continue;
        };

        keyframes.push(RawKeyframe {
            offset,
            path,
            renderable: true,
            timing_function,
        });
    }

    Some(Baked {
        keyframes,
        multi_param: true,
    })
}

/// Bakes each `points` keyframe by parsing it as a point list.
fn bake_points(
    element_tag: EId,
    points_keyframes: &[&str],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
) -> Option<Baked> {
    let closed = matches!(element_tag, EId::Polygon);

    let mut keyframes = Vec::new();
    for (i, &raw) in points_keyframes.iter().enumerate() {
        let offset = *key_offsets.get(i)?;
        let timing_function = key_timing_fns.get(i).copied().flatten();

        let points = parse_points_list(raw);
        let Some(path) = polyline_path(&points, closed) else {
            warn_invalid_geometry_value(raw);
            continue;
        };

        keyframes.push(RawKeyframe {
            offset,
            path,
            renderable: true,
            timing_function,
        });
    }

    Some(Baked {
        keyframes,
        multi_param: true,
    })
}

/// Builds a shape path from resolved geometry using the shared shape builders.
fn build_shape_path(element_tag: EId, g: &ShapeGeometry) -> Option<Path> {
    match element_tag {
        EId::Rect => rect_path(g.x, g.y, g.width, g.height, g.rx, g.ry),
        EId::Circle => circle_path(g.cx, g.cy, g.r),
        EId::Ellipse => ellipse_path(g.cx, g.cy, g.rx, g.ry),
        EId::Line => line_path(g.x1, g.y1, g.x2, g.y2),
        _ => None,
    }
}

/// Reports whether a shape produces a renderable (non-degenerate) snapshot.
fn is_shape_renderable(element_tag: EId, g: &ShapeGeometry) -> bool {
    match element_tag {
        EId::Rect => g.width.is_valid_length() && g.height.is_valid_length(),
        EId::Circle => g.r.is_valid_length(),
        EId::Ellipse => g.rx.is_valid_length() && g.ry.is_valid_length(),
        EId::Line => true,
        _ => false,
    }
}

/// Reports whether an attribute must be non-negative to bake a real shape.
fn is_non_negative_attribute(attribute_name: &str) -> bool {
    matches!(
        attribute_name,
        "width" | "height" | "r" | "rx" | "ry" | "fr"
    )
}

/// Reports whether every keyframe shares one verb sequence.
fn verbs_match(keyframes: &[RawKeyframe]) -> bool {
    let mut iter = keyframes.iter();
    let Some(first) = iter.next() else {
        return true;
    };
    iter.all(|k| k.path.verbs() == first.path.verbs())
}

/// Bakes the per-iteration accumulation offset for `accumulate=sum`.
///
/// Single-attribute tracks bake `end - base` point-wise; `d`/`points` cannot
/// accumulate and are dropped with a warning.
fn bake_accumulation(
    keyframes: &[RawKeyframe],
    accumulate: Accumulate,
    multi_param: bool,
    interpolable: bool,
    origin: Option<&Path>,
) -> Option<Arc<Path>> {
    if !matches!(accumulate, Accumulate::Sum) {
        return None;
    }

    if multi_param {
        warn_unsupported_accumulate();
        return None;
    }

    if !interpolable || keyframes.len() < 2 {
        return None;
    }

    let last = keyframes.last()?;
    subtract_paths(origin?, &last.path).map(Arc::new)
}

/// Builds `end - base` point-wise, preserving the shared verb sequence.
fn subtract_paths(base: &Path, end: &Path) -> Option<Path> {
    let mut builder = PathBuilder::new();
    let mut base_iter = base.segments();
    let mut end_iter = end.segments();

    loop {
        match (base_iter.next(), end_iter.next()) {
            (Some(b), Some(e)) => match (b, e) {
                (PathSegment::MoveTo(bp), PathSegment::MoveTo(ep)) => {
                    builder.move_to(ep.x - bp.x, ep.y - bp.y);
                }
                (PathSegment::LineTo(bp), PathSegment::LineTo(ep)) => {
                    builder.line_to(ep.x - bp.x, ep.y - bp.y);
                }
                (PathSegment::QuadTo(bp1, bp), PathSegment::QuadTo(ep1, ep)) => {
                    builder.quad_to(ep1.x - bp1.x, ep1.y - bp1.y, ep.x - bp.x, ep.y - bp.y);
                }
                (PathSegment::CubicTo(bp1, bp2, bp), PathSegment::CubicTo(ep1, ep2, ep)) => {
                    builder.cubic_to(
                        ep1.x - bp1.x,
                        ep1.y - bp1.y,
                        ep2.x - bp2.x,
                        ep2.y - bp2.y,
                        ep.x - bp.x,
                        ep.y - bp.y,
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

/// Parses absolute path data into a path, mirroring `shapes::convert_path`.
fn parse_path_data(data: &str) -> Option<Path> {
    let mut builder = PathBuilder::new();
    for segment in svgtypes::SimplifyingPathParser::from(data) {
        let Ok(segment) = segment else { break };
        match segment {
            svgtypes::SimplePathSegment::MoveTo { x, y } => builder.move_to(x as f32, y as f32),
            svgtypes::SimplePathSegment::LineTo { x, y } => builder.line_to(x as f32, y as f32),
            svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                builder.quad_to(x1 as f32, y1 as f32, x as f32, y as f32)
            }
            svgtypes::SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => builder.cubic_to(
                x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
            ),
            svgtypes::SimplePathSegment::ClosePath => builder.close(),
        }
    }
    builder.finish()
}

/// Parses a `points` list into an ordered point vector.
fn parse_points_list(data: &str) -> Vec<Point> {
    svgtypes::PointsParser::from(data)
        .map(|(x, y)| Point::from_xy(x as f32, y as f32))
        .collect()
}

fn warn_not_interpolable() {
    log::warn!("Animation values are not interpolable; using discrete interpolation.");
}

fn warn_invalid_geometry_value(value: impl std::fmt::Display) {
    log::warn!("Invalid geometry animation value: '{}'.", value);
}

fn warn_unsupported_accumulate() {
    log::warn!("Unsupported accumulate value; ignoring.");
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
