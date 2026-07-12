// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::too_many_arguments)]

use std::sync::Arc;

use svgtypes::AspectRatio;

use crate::{FillRule, LineCap, LineJoin, Opacity, Paint, Size, StrokeMiterlimit};

use super::Animation;

/// The carrier state for a path animation on a node.
#[derive(Clone, Copy, Debug)]
pub struct PathCarrierState {
    pub(crate) underlying_renderable: bool,
}

impl PathCarrierState {
    /// Creates a new `PathCarrierState`.
    pub fn new(underlying_renderable: bool) -> Self {
        Self {
            underlying_renderable,
        }
    }

    /// Whether the original static geometry was renderable.
    pub fn underlying_renderable(&self) -> bool {
        self.underlying_renderable
    }
}

/// The carrier state for a fill animation on a node.
#[derive(Clone, Debug)]
pub struct FillCarrierState {
    pub(crate) underlying_disabled: bool,
    pub(crate) paint: Option<Paint>,
    pub(crate) opacity: Opacity,
    pub(crate) rule: FillRule,
}

impl FillCarrierState {
    /// Creates a new `FillCarrierState`.
    pub fn new(
        underlying_disabled: bool,
        paint: Option<Paint>,
        opacity: Opacity,
        rule: FillRule,
    ) -> Self {
        Self {
            underlying_disabled,
            paint,
            opacity,
            rule,
        }
    }

    /// Whether the static fill was `none` or absent.
    pub fn underlying_disabled(&self) -> bool {
        self.underlying_disabled
    }

    /// The pre-resolved underlying fill paint, if any.
    pub fn paint(&self) -> Option<&Paint> {
        self.paint.as_ref()
    }

    /// The underlying fill opacity.
    pub fn opacity(&self) -> Opacity {
        self.opacity
    }

    /// The underlying fill rule.
    pub fn rule(&self) -> FillRule {
        self.rule
    }
}

/// The carrier state for a stroke animation on a node.
#[derive(Clone, Debug)]
pub struct StrokeCarrierState {
    pub(crate) underlying_disabled: bool,
    pub(crate) paint: Option<Paint>,
    pub(crate) opacity: Opacity,
    pub(crate) width: f32,
    pub(crate) linecap: LineCap,
    pub(crate) linejoin: LineJoin,
    pub(crate) miterlimit: StrokeMiterlimit,
    pub(crate) dasharray: Option<Vec<f32>>,
    pub(crate) dashoffset: f32,
}

impl StrokeCarrierState {
    /// Creates a new `StrokeCarrierState`.
    pub fn new(
        underlying_disabled: bool,
        paint: Option<Paint>,
        opacity: Opacity,
        width: f32,
        linecap: LineCap,
        linejoin: LineJoin,
        miterlimit: StrokeMiterlimit,
        dasharray: Option<Vec<f32>>,
        dashoffset: f32,
    ) -> Self {
        Self {
            underlying_disabled,
            paint,
            opacity,
            width,
            linecap,
            linejoin,
            miterlimit,
            dasharray,
            dashoffset,
        }
    }

    /// Whether the static stroke was `none` or absent.
    pub fn underlying_disabled(&self) -> bool {
        self.underlying_disabled
    }

    /// The pre-resolved underlying stroke paint, if any.
    pub fn paint(&self) -> Option<&Paint> {
        self.paint.as_ref()
    }

    /// The underlying stroke opacity.
    pub fn opacity(&self) -> Opacity {
        self.opacity
    }

    /// The underlying stroke width (may be zero).
    pub fn width(&self) -> f32 {
        self.width
    }

    /// The underlying stroke linecap.
    pub fn linecap(&self) -> LineCap {
        self.linecap
    }

    /// The underlying stroke linejoin.
    pub fn linejoin(&self) -> LineJoin {
        self.linejoin
    }

    /// The underlying stroke miterlimit.
    pub fn miterlimit(&self) -> StrokeMiterlimit {
        self.miterlimit
    }

    /// The underlying stroke dasharray, if any.
    pub fn dasharray(&self) -> Option<&[f32]> {
        self.dasharray.as_deref()
    }

    /// The underlying stroke dashoffset.
    pub fn dashoffset(&self) -> f32 {
        self.dashoffset
    }
}

/// The carrier state for an image geometry animation.
#[derive(Clone, Copy, Debug)]
pub struct ImageCarrierState {
    pub(crate) underlying_renderable: bool,
    pub(crate) static_quad: (f32, f32, f32, f32),
    pub(crate) aspect: AspectRatio,
    pub(crate) intrinsic_size: Size,
}

impl ImageCarrierState {
    /// Creates a new `ImageCarrierState`.
    pub fn new(
        underlying_renderable: bool,
        static_quad: (f32, f32, f32, f32),
        aspect: AspectRatio,
        intrinsic_size: Size,
    ) -> Self {
        Self {
            underlying_renderable,
            static_quad,
            aspect,
            intrinsic_size,
        }
    }

    /// Whether the original static image geometry was renderable.
    pub fn underlying_renderable(&self) -> bool {
        self.underlying_renderable
    }

    /// The static `(x, y, width, height)` quad.
    pub fn static_quad(&self) -> (f32, f32, f32, f32) {
        self.static_quad
    }

    /// The static `preserveAspectRatio`.
    pub fn aspect(&self) -> AspectRatio {
        self.aspect
    }

    /// The intrinsic image size.
    pub fn intrinsic_size(&self) -> Size {
        self.intrinsic_size
    }
}

/// All animations attached to a single node.
#[derive(Clone, Debug)]
pub struct NodeAnimation {
    pub(crate) animations: Vec<Arc<Animation>>,
    pub(crate) base_hidden: bool,
    pub(crate) path: Option<PathCarrierState>,
    pub(crate) fill: Option<FillCarrierState>,
    pub(crate) stroke: Option<StrokeCarrierState>,
    pub(crate) image: Option<ImageCarrierState>,
}

impl NodeAnimation {
    /// Creates a new `NodeAnimation`.
    pub fn new(
        animations: Vec<Arc<Animation>>,
        base_hidden: bool,
        path: Option<PathCarrierState>,
        fill: Option<FillCarrierState>,
        stroke: Option<StrokeCarrierState>,
        image: Option<ImageCarrierState>,
    ) -> Self {
        Self {
            animations,
            base_hidden,
            path,
            fill,
            stroke,
            image,
        }
    }

    /// The animations on this node.
    pub fn animations(&self) -> &[Arc<Animation>] {
        &self.animations
    }

    /// Whether the node was `display:none` in the static document.
    pub fn base_hidden(&self) -> bool {
        self.base_hidden
    }

    /// The path carrier state, if any.
    pub fn path(&self) -> Option<&PathCarrierState> {
        self.path.as_ref()
    }

    /// The fill carrier state, if any.
    pub fn fill(&self) -> Option<&FillCarrierState> {
        self.fill.as_ref()
    }

    /// The stroke carrier state, if any.
    pub fn stroke(&self) -> Option<&StrokeCarrierState> {
        self.stroke.as_ref()
    }

    /// The image carrier state, if any.
    pub fn image(&self) -> Option<&ImageCarrierState> {
        self.image.as_ref()
    }
}
