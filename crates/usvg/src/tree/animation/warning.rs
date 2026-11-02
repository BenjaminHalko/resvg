// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;

/// What an animation the parser couldn't take as written became.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnimationFidelity {
    /// Rendered as the specification says, through a fallback it defines
    /// (for example discrete stepping between values that can't be interpolated).
    Exact,
    /// Rendered, but not as written: a part was ignored.
    Approximate,
    /// Not rendered at all.
    Dropped,
}

/// The kind of loss an [`AnimationWarning`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnimationWarningCode {
    /// An animation of a `text` element or its content.
    TextAnimation,
    /// An animation inside a `filter`.
    FilterContentAnimation,
    /// A second, or an additive, `viewBox` animation.
    ExtraViewBoxAnimation,
    /// An `animation-name` with no `@keyframes` rule.
    UnknownKeyframes,
    /// A CSS custom property or `var()` in keyframes.
    CssVariable,
    /// A CSS property in keyframes that isn't animated.
    UnsupportedCssProperty,
    /// An `@keyframes` block without its closing brace.
    UnterminatedKeyframes,
    /// An `@import` of external CSS.
    ExternalCss,
    /// `accumulate="sum"` on an animation that can't accumulate.
    UnsupportedAccumulate,
    /// Values that can't be interpolated, stepped discretely instead.
    NotInterpolable,
    /// A value that doesn't parse.
    InvalidValue,
    /// A geometry value that doesn't parse.
    InvalidGeometryValue,
    /// An `animateMotion` without a `path` attribute (`mpath`, `values`, `from`, `to`, `by`).
    UnsupportedMotionForm,
    /// An `animateMotion` path or `keyPoints` that doesn't parse or fit.
    InvalidMotionPath,
    /// A gradient stop's `stop-color` that doesn't parse, taken as black.
    InvalidStopColor,
    /// `keyTimes` or `keySplines` that don't parse or fit.
    InvalidTiming,
    /// A SMIL timing attribute that isn't applied (`min`, `max`).
    UnsupportedTimingAttribute,
    /// A `begin` or `end` value that can't be resolved (an event, an access key, a wall clock).
    UnsupportedBeginEnd,
    /// An attribute that isn't animated.
    UnsupportedAttribute,
    /// A paint value that can't be animated (a paint server, `context-fill`).
    UnsupportedPaint,
    /// An `animation` shorthand that doesn't parse.
    InvalidAnimationShorthand,
    /// A `stroke-width` animation on an element without a stroke, which draws nothing.
    StrokeWidthWithoutStroke,
}

impl AnimationWarningCode {
    /// What the animation the warning names became.
    pub fn fidelity(self) -> AnimationFidelity {
        match self {
            Self::NotInterpolable | Self::StrokeWidthWithoutStroke => AnimationFidelity::Exact,
            Self::UnsupportedAccumulate
            | Self::InvalidStopColor
            | Self::UnsupportedTimingAttribute => AnimationFidelity::Approximate,
            Self::TextAnimation
            | Self::FilterContentAnimation
            | Self::ExtraViewBoxAnimation
            | Self::UnknownKeyframes
            | Self::CssVariable
            | Self::UnsupportedCssProperty
            | Self::UnterminatedKeyframes
            | Self::ExternalCss
            | Self::InvalidValue
            | Self::InvalidGeometryValue
            | Self::UnsupportedMotionForm
            | Self::InvalidMotionPath
            | Self::InvalidTiming
            | Self::UnsupportedBeginEnd
            | Self::UnsupportedAttribute
            | Self::UnsupportedPaint
            | Self::InvalidAnimationShorthand => AnimationFidelity::Dropped,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::TextAnimation => "Animation of text elements is not supported",
            Self::FilterContentAnimation => "Animation of filter content is not supported",
            Self::ExtraViewBoxAnimation => {
                "Only a single non-additive viewBox animation is supported"
            }
            Self::UnknownKeyframes => "Unknown keyframes name",
            Self::CssVariable => "CSS variables are not supported",
            Self::UnsupportedCssProperty => "Unsupported CSS property in keyframes",
            Self::UnterminatedKeyframes => "Unterminated @keyframes block; skipped",
            Self::ExternalCss => "External CSS is not supported",
            Self::UnsupportedAccumulate => "Unsupported accumulate value; ignoring",
            Self::NotInterpolable => {
                "Animation values are not interpolable; using discrete interpolation"
            }
            Self::InvalidValue => "Invalid animation value",
            Self::InvalidGeometryValue => "Invalid geometry animation value",
            Self::UnsupportedMotionForm => "Unsupported animateMotion form",
            Self::InvalidMotionPath => "Invalid animateMotion path",
            Self::InvalidStopColor => "Failed to parse stop-color value",
            Self::InvalidTiming => "Invalid animation timing",
            Self::UnsupportedTimingAttribute => "Unsupported SMIL timing attribute",
            Self::UnsupportedBeginEnd => "Unsupported animation begin/end value",
            Self::UnsupportedAttribute => "Unsupported animation attribute",
            Self::UnsupportedPaint => "Unsupported paint value",
            Self::InvalidAnimationShorthand => "Invalid animation shorthand",
            Self::StrokeWidthWithoutStroke => {
                "Stroke width animation on an element without a stroke has no effect"
            }
        }
    }
}

/// An animation the parser couldn't take as written.
///
/// Collected per tree, each distinct warning once, in the order met; see
/// [`Tree::animation_warnings`](crate::Tree::animation_warnings).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnimationWarning {
    pub(crate) code: AnimationWarningCode,
    pub(crate) element: Option<String>,
    pub(crate) detail: Option<String>,
}

impl AnimationWarning {
    /// The kind of loss.
    pub fn code(&self) -> AnimationWarningCode {
        self.code
    }

    /// What the animation became.
    pub fn fidelity(&self) -> AnimationFidelity {
        self.code.fidelity()
    }

    /// The `id` of the animated element, when it has one and is known.
    pub fn element(&self) -> Option<&str> {
        self.element.as_deref()
    }

    /// The value, attribute or property named, when there is one.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

impl fmt::Display for AnimationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code.message())?;
        // The property a `var()` sits in is kept for callers; the log line names none.
        let shown = self.code != AnimationWarningCode::CssVariable;
        if let Some(detail) = self.detail.as_ref().filter(|_| shown) {
            write!(f, ": '{}'", detail)?;
        }
        f.write_str(".")
    }
}
