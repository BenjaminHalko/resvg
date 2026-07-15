// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{FillRule, LineCap, LineJoin, NonZeroRect, NormalizedF32, Opacity, StrokeMiterlimit};

use super::{
    CssBox, CssOrigin, CssOriginBounds, Easing, MotionTrack, OriginComponent, PathTrack, Timing,
    Track, TransformFunction,
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

/// The animated component of a gradient's geometry.
///
/// The component is retained alongside resolved user-unit keyframes so renderers
/// never infer it from matching scalar values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientGeometryComponent {
    /// A linear gradient's `x1` endpoint.
    LinearX1,
    /// A linear gradient's `y1` endpoint.
    LinearY1,
    /// A linear gradient's `x2` endpoint.
    LinearX2,
    /// A linear gradient's `y2` endpoint.
    LinearY2,
    /// A radial gradient's center x coordinate.
    RadialCx,
    /// A radial gradient's center y coordinate.
    RadialCy,
    /// A radial gradient's radius.
    RadialR,
    /// A radial gradient's focal x coordinate.
    RadialFx,
    /// A radial gradient's focal y coordinate.
    RadialFy,
    /// A radial gradient's focal radius.
    RadialFr,
}

impl GradientGeometryComponent {
    pub(crate) fn from_attribute_name(name: &str) -> Option<Self> {
        match name {
            "x1" => Some(Self::LinearX1),
            "y1" => Some(Self::LinearY1),
            "x2" => Some(Self::LinearX2),
            "y2" => Some(Self::LinearY2),
            "cx" => Some(Self::RadialCx),
            "cy" => Some(Self::RadialCy),
            "r" => Some(Self::RadialR),
            "fx" => Some(Self::RadialFx),
            "fy" => Some(Self::RadialFy),
            "fr" => Some(Self::RadialFr),
            _ => None,
        }
    }
}

/// Resolved scalar keyframes for one typed gradient geometry component.
#[derive(Clone, Debug)]
pub struct GradientGeometry {
    pub(crate) component: GradientGeometryComponent,
    pub(crate) track: Track<f32>,
}

impl GradientGeometry {
    pub(crate) fn new(component: GradientGeometryComponent, track: Track<f32>) -> Self {
        Self { component, track }
    }

    /// The gradient geometry component represented by this track.
    pub fn component(&self) -> GradientGeometryComponent {
        self.component
    }

    /// The resolved scalar keyframes for this component.
    pub fn track(&self) -> &Track<f32> {
        &self.track
    }
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
    /// A resolved geometry scalar awaiting target-specific lowering.
    ///
    /// This is a parser intermediate and is never attached to a rendered shape,
    /// image, or gradient.
    Geometry(Track<f32>),
    /// An animated `stop-color`.
    StopColor(Track<svgtypes::Color>),
    /// An animated `stop-opacity`.
    StopOpacity(Track<Opacity>),
    /// An animated `offset` on a gradient stop.
    StopOffset(Track<NormalizedF32>),
    /// An animated, resolved gradient geometry component.
    GradientGeometry(GradientGeometry),
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
    pub(crate) fn bake_css_origin(&mut self, bounds: CssOriginBounds) {
        let Some(origin) = self.css_origin.take() else {
            return;
        };
        let AnimationKind::Transform(track) = &mut self.kind else {
            return;
        };
        let bounds = match origin.box_ {
            CssBox::Stroke => bounds.stroke,
            CssBox::Content | CssBox::Border | CssBox::Fill => bounds.fill,
            CssBox::View => bounds.view,
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
        OriginComponent::Absolute(value) => value,
    }
}
