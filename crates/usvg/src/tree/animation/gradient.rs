// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::NormalizedF32;

use super::Animation;

/// A source stop for gradient animation.
#[derive(Clone, Debug)]
pub struct SourceStop {
    pub(crate) base_offset: NormalizedF32,
    pub(crate) synthesized: bool,
    pub(crate) animations: Vec<Arc<Animation>>,
}

impl SourceStop {
    /// Creates a new `SourceStop`.
    pub fn new(
        base_offset: NormalizedF32,
        synthesized: bool,
        animations: Vec<Arc<Animation>>,
    ) -> Self {
        Self {
            base_offset,
            synthesized,
            animations,
        }
    }

    /// The unmodified source offset.
    pub fn base_offset(&self) -> NormalizedF32 {
        self.base_offset
    }

    /// Whether this stop was synthesized (not present in the source document).
    pub fn synthesized(&self) -> bool {
        self.synthesized
    }

    /// The animations on this stop.
    pub fn animations(&self) -> &[Arc<Animation>] {
        &self.animations
    }
}

/// All animations attached to a gradient.
#[derive(Clone, Debug)]
pub struct GradientAnimation {
    pub(crate) animations: Vec<Arc<Animation>>,
    pub(crate) underlying_r: Option<f32>,
    pub(crate) focal_x_is_omitted: bool,
    pub(crate) focal_y_is_omitted: bool,
    pub(crate) source_stops: Vec<SourceStop>,
    pub(crate) source_indices: Vec<Option<usize>>,
}

impl GradientAnimation {
    /// Creates a new `GradientAnimation`.
    pub fn new(
        animations: Vec<Arc<Animation>>,
        underlying_r: Option<f32>,
        focal_x_is_omitted: bool,
        focal_y_is_omitted: bool,
        source_stops: Vec<SourceStop>,
        source_indices: Vec<Option<usize>>,
    ) -> Self {
        Self {
            animations,
            underlying_r,
            focal_x_is_omitted,
            focal_y_is_omitted,
            source_stops,
            source_indices,
        }
    }

    /// The gradient-level animations.
    pub fn animations(&self) -> &[Arc<Animation>] {
        &self.animations
    }

    /// The true static radius for a synthesized radial carrier, if any.
    pub fn underlying_r(&self) -> Option<f32> {
        self.underlying_r
    }

    /// Whether `fx` was absent after radial-gradient href resolution.
    pub fn focal_x_is_omitted(&self) -> bool {
        self.focal_x_is_omitted
    }

    /// Whether `fy` was absent after radial-gradient href resolution.
    pub fn focal_y_is_omitted(&self) -> bool {
        self.focal_y_is_omitted
    }

    /// The source stops.
    pub fn source_stops(&self) -> &[SourceStop] {
        &self.source_stops
    }

    /// Returns the source stop index for a given converted stop index.
    pub fn source_index_of(&self, stop_index: usize) -> Option<usize> {
        self.source_indices.get(stop_index).copied().flatten()
    }
}
