// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{FillRule, LineCap, LineJoin, NonZeroRect, NormalizedF32, Opacity, StrokeMiterlimit};

use super::{Easing, MotionTrack, PathTrack, Timing, Track, TransformTrack};

/// The source of an animation — SMIL or CSS.
#[derive(Clone, Copy, Debug)]
pub enum AnimationSource {
    /// A SMIL animation element (`<animate>`, `<animateTransform>`, etc.).
    Smil,
    /// A CSS `@keyframes` animation.
    Css,
}

/// Whether an animation adds to or replaces the underlying value.
#[derive(Clone, Copy, Debug)]
pub enum Additive {
    /// The animation value replaces the underlying value.
    Replace,
    /// The animation value is added to the underlying value.
    Sum,
}

/// Whether an animation accumulates across iterations.
#[derive(Clone, Copy, Debug)]
pub enum Accumulate {
    /// No accumulation.
    None,
    /// Accumulate across iterations.
    Sum,
}

/// The visibility value for animation (distinct from the private `usvg::Visibility`).
#[derive(Clone, Copy, Debug)]
pub enum AnimationVisibility {
    /// The element is visible.
    Visible,
    /// The element is hidden.
    Hidden,
    /// The element is collapsed.
    Collapse,
}

/// The kind of an animation, carrying its typed keyframe data.
#[derive(Clone, Debug)]
pub enum AnimationKind {
    /// An animated `transform` (SMIL or CSS).
    Transform(TransformTrack),
    /// An animated `gradientTransform`.
    GradientTransform(TransformTrack),
    /// An `animateMotion` path animation.
    Motion(MotionTrack),
    /// An animated `opacity`.
    Opacity(Track<Opacity>),
    /// An animated `fill` color.
    Fill(Track<svgtypes::Color>),
    /// An animated `stroke` color.
    Stroke(Track<svgtypes::Color>),
    /// An animated `stroke-width` (non-negative; zero is allowed).
    StrokeWidth(Track<f32>),
    /// An animated `stroke-dashoffset`.
    StrokeDashoffset(Track<f32>),
    /// An animated `stroke-dasharray`.
    StrokeDasharray(Track<Vec<f32>>),
    /// An animated `stroke-miterlimit`.
    StrokeMiterlimit(Track<StrokeMiterlimit>),
    /// An animated `stroke-linecap`.
    StrokeLinecap(Track<LineCap>),
    /// An animated `stroke-linejoin`.
    StrokeLinejoin(Track<LineJoin>),
    /// An animated `fill-rule`.
    FillRule(Track<FillRule>),
    /// An animated `display`.
    Display(Track<bool>),
    /// An animated `visibility`.
    Visibility(Track<AnimationVisibility>),
    /// A baked geometry animation.
    Path(PathTrack),
    /// An animated `stop-color`.
    StopColor(Track<svgtypes::Color>),
    /// An animated `stop-opacity`.
    StopOpacity(Track<Opacity>),
    /// An animated `offset` on a gradient stop.
    StopOffset(Track<NormalizedF32>),
    /// An animated gradient geometry scalar.
    GradientGeometry(Track<f32>),
    /// An animated `viewBox` rect.
    ViewBox(Track<NonZeroRect>),
    /// An animated `image x` position.
    ImageX(Track<f32>),
    /// An animated `image y` position.
    ImageY(Track<f32>),
    /// An animated `image width`.
    ImageWidth(Track<f32>),
    /// An animated `image height`.
    ImageHeight(Track<f32>),
}

/// A complete animation with timing, easing, and kind.
#[derive(Clone, Debug)]
pub struct Animation {
    pub(crate) kind: AnimationKind,
    pub(crate) timing: Timing,
    pub(crate) easing: Easing,
    pub(crate) additive: Additive,
    pub(crate) accumulate: Accumulate,
    pub(crate) source: AnimationSource,
    pub(crate) suppressed_by_important: bool,
}

impl Animation {
    /// Creates a new `Animation`.
    pub fn new(
        kind: AnimationKind,
        timing: Timing,
        easing: Easing,
        additive: Additive,
        accumulate: Accumulate,
        source: AnimationSource,
        suppressed_by_important: bool,
    ) -> Self {
        Self {
            kind,
            timing,
            easing,
            additive,
            accumulate,
            source,
            suppressed_by_important,
        }
    }

    /// The animation kind and keyframe data.
    pub fn kind(&self) -> &AnimationKind {
        &self.kind
    }

    /// The animation timing.
    pub fn timing(&self) -> &Timing {
        &self.timing
    }

    /// The easing parameters.
    pub fn easing(&self) -> &Easing {
        &self.easing
    }

    /// Whether the animation adds to or replaces the underlying value.
    pub fn additive(&self) -> Additive {
        self.additive
    }

    /// Whether the animation accumulates across iterations.
    pub fn accumulate(&self) -> Accumulate {
        self.accumulate
    }

    /// The animation source (SMIL or CSS).
    pub fn source(&self) -> AnimationSource {
        self.source
    }

    /// Whether this animation is suppressed by an `!important` static declaration.
    pub fn suppressed_by_important(&self) -> bool {
        self.suppressed_by_important
    }

    /// The end time of the animation's first loop, in seconds.
    ///
    /// Repeats and infinite iterations collapse to a single loop, so an
    /// indefinitely repeating animation reports the length of one cycle. An
    /// animation with an indefinite simple duration contributes `None`.
    pub(crate) fn one_loop_end(&self) -> Option<f32> {
        self.timing.one_loop_end()
    }
}
