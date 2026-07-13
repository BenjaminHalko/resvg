// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    FillRule, LineCap, LineJoin, NonZeroRect, NormalizedF32, Opacity, Rect, StrokeMiterlimit,
};

use super::{
    CssBox, CssOrigin, Easing, MotionTrack, OriginComponent, PathTrack, Timing, Track,
    TransformFunction,
};

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
    /// An animated `transform` (SMIL or CSS), lowered to a function-list track.
    ///
    /// CSS `transform-origin` is baked into constant translate wrappers after
    /// bounding boxes are resolved. This relies on CSS transform animations
    /// using replace composition without accumulation.
    Transform(Track<Vec<TransformFunction>>),
    /// An animated `gradientTransform` with no CSS origin wrappers.
    GradientTransform(Track<Vec<TransformFunction>>),
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
    pub(crate) css_origin: Option<CssOrigin>,
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
            css_origin: None,
        }
    }

    /// Retains a CSS transform origin until the target's bounds are known.
    pub(crate) fn with_css_origin(mut self, origin: CssOrigin) -> Self {
        self.css_origin = Some(origin);
        self
    }

    /// Bakes a CSS transform origin into each transform-function keyframe.
    pub(crate) fn bake_css_origin(&mut self, fill_bounds: Rect, stroke_bounds: Rect) {
        let Some(origin) = self.css_origin.take() else {
            return;
        };
        let AnimationKind::Transform(track) = &mut self.kind else {
            return;
        };
        let bounds = match origin.box_ {
            CssBox::Stroke => stroke_bounds,
            CssBox::Content | CssBox::Border | CssBox::Fill | CssBox::View => fill_bounds,
        };
        let x = resolve_origin_component(origin.x, bounds.x(), bounds.width());
        let y = resolve_origin_component(origin.y, bounds.y(), bounds.height());
        for keyframe in &mut track.keyframes {
            let mut functions = Vec::with_capacity(keyframe.value.len() + 2);
            functions.push(TransformFunction::Translate(x, y));
            functions.append(&mut keyframe.value);
            functions.push(TransformFunction::Translate(-x, -y));
            keyframe.value = functions;
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

fn resolve_origin_component(component: OriginComponent, offset: f32, extent: f32) -> f32 {
    match component {
        OriginComponent::Length(value) => offset + value,
        OriginComponent::Percent(value) => offset + extent * value / 100.0,
    }
}
