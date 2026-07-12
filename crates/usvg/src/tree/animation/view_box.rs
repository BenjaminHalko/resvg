// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use svgtypes::AspectRatio;

use crate::{NonZeroRect, Size, Transform};

use super::{Easing, Timing, Track};

/// A viewBox animation on the root SVG element.
#[derive(Clone, Debug)]
pub struct ViewBoxAnimation {
    pub(crate) track: Track<NonZeroRect>,
    pub(crate) static_aspect: AspectRatio,
    pub(crate) timing: Timing,
    pub(crate) easing: Easing,
}

impl ViewBoxAnimation {
    /// Creates a new `ViewBoxAnimation`.
    pub fn new(
        track: Track<NonZeroRect>,
        static_aspect: AspectRatio,
        timing: Timing,
        easing: Easing,
    ) -> Self {
        Self {
            track,
            static_aspect,
            timing,
            easing,
        }
    }

    /// The viewBox keyframe track.
    pub fn track(&self) -> &Track<NonZeroRect> {
        &self.track
    }

    /// The static `preserveAspectRatio`.
    pub fn static_aspect(&self) -> AspectRatio {
        self.static_aspect
    }

    /// The animation timing.
    pub fn timing(&self) -> &Timing {
        &self.timing
    }

    /// The easing parameters.
    pub fn easing(&self) -> &Easing {
        &self.easing
    }

    /// Computes the root transform for a sampled `viewBox` rect.
    ///
    /// `viewBox` in SVG.
    pub fn to_transform(&self, sampled_rect: NonZeroRect, tree_size: Size) -> Transform {
        super::super::geom::ViewBox {
            rect: sampled_rect,
            aspect: self.static_aspect,
        }
        .to_transform(tree_size)
    }
}
