// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tiny_skia_path::{Path, PathBuilder, PathSegment};

use crate::tree::animation::Accumulate;

use super::keyframes::RawKeyframe;

/// Reports whether every keyframe shares one verb sequence.
pub(super) fn verbs_match(keyframes: &[RawKeyframe]) -> bool {
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
pub(super) fn bake_accumulation(
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
        log::warn!("Unsupported accumulate value; ignoring.");
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
