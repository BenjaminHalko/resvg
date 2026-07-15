// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia_path::{Path, PathBuilder, Point};

use crate::NormalizedF32;
use crate::parser::shapes::polyline_path;
use crate::parser::svgtree::EId;
use crate::tree::animation::TimingFunction;

/// A baked keyframe before its path is shared behind an `Arc`.
pub(super) struct RawKeyframe {
    pub(super) offset: NormalizedF32,
    pub(super) path: Path,
    pub(super) renderable: bool,
    pub(super) timing_function: Option<TimingFunction>,
}

/// A baked keyframe sequence.
pub(super) struct Baked {
    pub(super) keyframes: Vec<RawKeyframe>,
    /// Whether the value interpolates multiple parameters (`d`/`points`), which
    /// cannot accumulate.
    pub(super) multi_param: bool,
}

/// Bakes each `d` keyframe by parsing it as absolute path data.
pub(super) fn bake_path_data(
    d_keyframes: &[&str],
    key_offsets: &[NormalizedF32],
    key_timing_fns: &[Option<TimingFunction>],
) -> Option<Baked> {
    let mut keyframes = Vec::new();
    for (i, &raw) in d_keyframes.iter().enumerate() {
        let offset = *key_offsets.get(i)?;
        let timing_function = key_timing_fns.get(i).copied().flatten();

        let Some((path, renderable)) = parse_path_data(raw) else {
            warn_invalid_geometry_value(raw);
            continue;
        };

        keyframes.push(RawKeyframe {
            offset,
            path,
            renderable,
            timing_function,
        });
    }

    Some(Baked {
        keyframes,
        multi_param: true,
    })
}

/// Bakes each `points` keyframe by parsing it as a point list.
pub(super) fn bake_points(
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

        let points: Vec<_> = svgtypes::PointsParser::from(raw)
            .map(|(x, y)| Point::from_xy(x as f32, y as f32))
            .collect();
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

/// Parses absolute path data into a path, mirroring `shapes::convert_path`.
fn parse_path_data(data: &str) -> Option<(Path, bool)> {
    let mut builder = PathBuilder::new();
    let mut last_move = None;
    let mut renderable = false;
    for segment in svgtypes::SimplifyingPathParser::from(data) {
        let Ok(segment) = segment else { return None };
        match segment {
            svgtypes::SimplePathSegment::MoveTo { x, y } => {
                let point = (x as f32, y as f32);
                builder.move_to(point.0, point.1);
                last_move = Some(point);
            }
            svgtypes::SimplePathSegment::LineTo { x, y } => {
                builder.line_to(x as f32, y as f32);
                renderable = true;
            }
            svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                builder.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
                renderable = true;
            }
            svgtypes::SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                builder.cubic_to(
                    x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
                );
                renderable = true;
            }
            svgtypes::SimplePathSegment::ClosePath => builder.close(),
        }
    }
    if !renderable {
        let (x, y) = last_move?;
        builder.line_to(x, y);
    }
    builder.finish().map(|path| (path, renderable))
}

pub(super) fn warn_invalid_geometry_value(value: impl std::fmt::Display) {
    log::warn!("Invalid geometry animation value: '{}'.", value);
}
