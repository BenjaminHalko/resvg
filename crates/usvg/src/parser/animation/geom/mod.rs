// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Baking of geometry attribute animations into path-data keyframe tracks.
//!
//! Shape geometry attributes (`width`, `r`, `cx`, ...) and the `d`/`points`
//! attributes are baked into a [`PathTrack`] at parse time by substituting each
//! keyframe value into the corresponding shape builder. A keyframe sequence is
//! point-wise interpolable only when every snapshot shares one verb sequence;
//! otherwise the track falls back to discrete stepping.

use crate::tree::animation::{AnimationKind, CalcMode};

mod accumulate;
mod bake;
mod keyframes;
mod shape;

/// The result of baking a geometry animation into a path track.
pub(crate) struct GeometryBake {
    /// The baked path track.
    pub(crate) kind: AnimationKind,
    /// The calculation mode, forced to `Discrete` when keyframes are not
    /// point-wise interpolable.
    pub(crate) calc_mode: CalcMode,
}

pub(crate) use bake::{bake_geometry_animation, bake_geometry_animation_with_sum_base};
pub(crate) use shape::ShapeGeometry;
