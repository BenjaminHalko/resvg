// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tiny_skia::{Path, PathBuilder, PathSegment};
use usvg::{CalcMode, Easing, PathTrack, TimingFunction};

use super::locate::{lerp, locate};

/// Samples a baked path track point-wise, returning the shape and its
/// renderability.
///
/// The sampled shape renders unless both bracketing keyframes are degenerate or
/// the frame rests exactly on a degenerate keyframe with no progress toward a
/// renderable neighbor, so a `0 -> 100` grow draws at every `t > 0`.
pub(super) fn sample_path(
    track: &PathTrack,
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<(Arc<Path>, bool)> {
    let keyframes = track.keyframes();
    if keyframes.is_empty() {
        return None;
    }

    let paced = if matches!(easing.calc_mode(), CalcMode::Paced) {
        Some(
            (0..keyframes.len().saturating_sub(1))
                .map(|i| path_distance(keyframes[i].path(), keyframes[i + 1].path()))
                .collect::<Vec<f32>>(),
        )
    } else {
        None
    };

    let (lo, hi, t) = locate(
        keyframes,
        easing,
        timing_function,
        progress,
        paced.as_deref(),
    );

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
