// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ApproxEqUlps;
use std::time::Duration;

/// Represents an animated value that can be either a single static value or a sequence of keyframes.
#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub enum AnimatedValue<T> {
    /// A single static value (no animation).
    Static(T),
    /// A sequence of keyframes defining the animation.
    Animated(Vec<Keyframe<T>>),
}

/// A generic wrapper that makes any value potentially animatable.
#[derive(Clone, Debug)]
pub struct Animatable<T> {
    /// The base value (static or first keyframe).
    value: T,
    /// Animation data when available.
    #[cfg(feature = "animation")]
    animation: Option<AnimatedValue<T>>,
}

impl<T> Animatable<T> {
    /// Creates a new static animatable value.
    pub fn new(value: T) -> Self {
        Self {
            value,
            #[cfg(feature = "animation")]
            animation: None,
        }
    }

    /// Gets the base value (static value or first keyframe).
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Gets an owned copy of the base value.
    pub fn resolve(&self) -> T where T: Clone {
        self.value.clone()
    }

    /// Sets the base value.
    pub fn set(&mut self, value: T) {
        self.value = value;
    }

    /// Gets the animation data if available.
    #[cfg(feature = "animation")]
    pub fn animated(&self) -> Option<&AnimatedValue<T>> {
        self.animation.as_ref()
    }

    /// Gets the animation data if available.
    #[cfg(not(feature = "animation"))]
    pub fn animated(&self) -> Option<&T> {
        Some(&self.value)
    }

    /// Sets the animation data.
    #[cfg(feature = "animation")]
    pub fn set_animation(&mut self, animation: AnimatedValue<T>) {
        self.animation = Some(animation);
    }

    /// Gets the keyframes if this value is animated.
    #[cfg(feature = "animation")]
    pub fn keyframes(&self) -> Option<&[Keyframe<T>]> {
        self.animation.as_ref().map(|a| match a {
            AnimatedValue::Static(_) => &[],
            AnimatedValue::Animated(ref keyframes) => keyframes,
        })
    }

    /// Checks if this value has animation data.
    #[cfg(feature = "animation")]
    pub fn is_animated(&self) -> bool {
        self.animation.is_some()
    }

    /// Checks if this value has animation data.
    #[cfg(not(feature = "animation"))]
    pub fn is_animated(&self) -> bool {
        false
    }
}

impl<T> From<T> for Animatable<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Default for Animatable<T> where T: Default {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// List of ALL SVG properties that can be animated
/// This is a comprehensive reference for what should be made animatable
pub const ANIMATABLE_PROPERTIES: &[&str] = &[
    // Basic properties
    "opacity", "visibility", "display",

    // Transform properties
    "transform", "translate", "rotate", "scale", "skewX", "skewY", "matrix",

    // Fill properties
    "fill", "fill-opacity", "fill-rule",

    // Stroke properties
    "stroke", "stroke-width", "stroke-opacity", "stroke-dasharray", "stroke-dashoffset",
    "stroke-linecap", "stroke-linejoin", "stroke-miterlimit",

    // Color properties
    "color", "stop-color", "stop-opacity", "flood-color", "flood-opacity",
    "lighting-color",

    // Text properties
    "font-size", "font-family", "font-weight", "font-style", "font-variant",
    "font-stretch", "letter-spacing", "word-spacing", "text-decoration",
    "text-anchor", "baseline-shift",

    // Path properties
    "d", "pathLength",

    // Filter properties
    "filter", "feBlend", "feColorMatrix", "feComponentTransfer", "feComposite",
    "feConvolveMatrix", "feDiffuseLighting", "feDisplacementMap", "feDropShadow",
    "feFlood", "feGaussianBlur", "feImage", "feMerge", "feMorphology", "feOffset",
    "feSpecularLighting", "feTile", "feTurbulence",

    // Gradient properties
    "stop", "linearGradient", "radialGradient", "gradientTransform", "gradientUnits",

    // Animation properties
    "animate", "animateColor", "animateMotion", "animateTransform", "begin", "dur",
    "end", "repeatCount", "repeatDur", "restart", "fill", "calcMode", "values",
    "keyTimes", "keySplines", "from", "to", "by",

    // Other properties
    "clip-path", "mask", "viewBox", "preserveAspectRatio", "cx", "cy", "r", "rx", "ry",
    "x", "y", "width", "height", "x1", "y1", "x2", "y2", "points", "marker-start",
    "marker-mid", "marker-end", "markerHeight", "markerWidth", "markerUnits",
    "patternUnits", "patternContentUnits", "patternTransform"
];

/// API for accessing keyframes and animation data
/// Example usage:
/// ```rust
/// if let Some(animated_value) = fill.animated_opacity() {
///     match animated_value {
///         AnimatedValue::Static(value) => println!("Static: {:?}", value),
///         AnimatedValue::Animated(keyframes) => {
///             for keyframe in keyframes {
///                 println!("Time: {}, Value: {:?}", keyframe.time, keyframe.value);
///             }
///         }
///     }
/// }
/// ```


#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub struct Keyframe<T> {
    /// The time offset for this keyframe, as a fraction of the total animation duration (0.0 to 1.0).
    pub time: f32,
    /// The value at this keyframe.
    pub value: T,
    /// The timing function to use when interpolating to the next keyframe.
    pub timing_function: TimingFunction,
}

#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub enum TimingFunction {
    /// Linear interpolation.
    Linear,
    /// Ease timing function.
    Ease,
    /// Ease-in timing function.
    EaseIn,
    /// Ease-out timing function.
    EaseOut,
    /// Ease-in-out timing function.
    EaseInOut,
    /// Cubic bezier timing function with custom control points.
    CubicBezier(f32, f32, f32, f32),
    /// Step timing function with the specified number of steps.
    Steps(u32, StepPosition),
}

#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub enum StepPosition {
    /// The step function jumps at the start of each step.
    JumpStart,
    /// The step function jumps at the end of each step.
    JumpEnd,
    /// The step function jumps at the middle of each step.
    JumpNone,
    /// The step function jumps at both the start and end of each step.
    JumpBoth,
}

#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub struct Animation {
    /// The duration of the animation.
    pub duration: Duration,
    /// The number of times the animation should repeat. 0 means infinite.
    pub iterations: u32,
    /// The direction of the animation.
    pub direction: AnimationDirection,
    /// The fill mode of the animation.
    pub fill_mode: FillMode,
    /// The delay before the animation starts.
    pub delay: Duration,
}

#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub enum AnimationDirection {
    /// Animation plays forward.
    Normal,
    /// Animation plays forward then backward.
    Alternate,
    /// Animation plays backward.
    Reverse,
    /// Animation plays backward then forward.
    AlternateReverse,
}

#[derive(Clone, Debug)]
#[cfg(feature = "animation")]
pub enum FillMode {
    /// The animation has no effect outside its duration.
    None,
    /// The animation applies the start values before the animation starts.
    Forwards,
    /// The animation applies the end values after the animation ends.
    Backwards,
    /// The animation applies both start and end values outside its duration.
    Both,
}

// Fallback implementation when animation feature is disabled
#[cfg(not(feature = "animation"))]
pub type AnimatedValue<T> = T;

impl<T> AnimatedValue<T> {
    /// Creates a new static animated value.
    #[cfg(feature = "animation")]
    pub fn new_static(value: T) -> Self {
        AnimatedValue::Static(value)
    }

    /// Creates a new animated value with keyframes.
    #[cfg(feature = "animation")]
    pub fn new_animated(keyframes: Vec<Keyframe<T>>) -> Self {
        AnimatedValue::Animated(keyframes)
    }

    /// Returns true if this is a static (non-animated) value.
    #[cfg(feature = "animation")]
    pub fn is_static(&self) -> bool {
        matches!(self, AnimatedValue::Static(_))
    }

    /// Returns true if this is an animated value.
    #[cfg(feature = "animation")]
    pub fn is_animated(&self) -> bool {
        matches!(self, AnimatedValue::Animated(_))
    }

    /// Gets the static value.
    /// If the value is animated, returns the value from the first keyframe.
    /// If there are no keyframes, returns the default value for T.
    #[cfg(feature = "animation")]
    pub fn as_static(&self) -> &T {
        match self {
            AnimatedValue::Static(ref value) => value,
            AnimatedValue::Animated(ref keyframes) => {
                keyframes.first().map(|keyframe| &keyframe.value).unwrap_or(&T::default())
            }
        }
    }

    /// Resolves the animated value to a concrete value.
    /// For static values, returns the value directly.
    /// For animated values, returns the first keyframe value or default.
    #[cfg(feature = "animation")]
    pub fn resolve(&self) -> T where T: Clone + Default {
        match self {
            AnimatedValue::Static(ref value) => value.clone(),
            AnimatedValue::Animated(ref keyframes) => {
                keyframes.first().map(|keyframe| keyframe.value.clone()).unwrap_or_default()
            }
        }
    }

    /// Gets the keyframes if this is an animated value.
    #[cfg(feature = "animation")]
    pub fn as_animated(&self) -> Option<&[Keyframe<T>]> {
        match self {
            AnimatedValue::Animated(ref keyframes) => Some(keyframes),
            _ => None,
        }
    }
}

// Fallback implementation when animation feature is disabled
#[cfg(not(feature = "animation"))]
impl<T> AnimatedValue<T> {
    pub fn new_static(value: T) -> T {
        value
    }

    pub fn is_static(&self) -> bool {
        true
    }

    pub fn is_animated(&self) -> bool {
        false
    }

    pub fn as_static(&self) -> &T {
        self
    }

    pub fn as_animated(&self) -> Option<&[Keyframe<T>]> {
        None
    }
}

#[cfg(feature = "animation")]
impl<T> Default for AnimatedValue<T>
where
    T: Default,
{
    fn default() -> Self {
        AnimatedValue::Static(T::default())
    }
}

#[cfg(not(feature = "animation"))]
impl<T> Default for AnimatedValue<T>
where
    T: Default,
{
    fn default() -> T {
        T::default()
    }
}

#[cfg(feature = "animation")]
impl<T> Keyframe<T> {
    /// Creates a new keyframe.
    pub fn new(time: f32, value: T) -> Self {
        Self {
            time,
            value,
            timing_function: TimingFunction::Linear,
        }
    }

    /// Creates a new keyframe with a custom timing function.
    pub fn new_with_timing(time: f32, value: T, timing_function: TimingFunction) -> Self {
        Self {
            time,
            value,
            timing_function,
        }
    }
}

#[cfg(feature = "animation")]
impl Default for TimingFunction {
    fn default() -> Self {
        TimingFunction::Linear
    }
}

#[cfg(feature = "animation")]
impl Default for AnimationDirection {
    fn default() -> Self {
        AnimationDirection::Normal
    }
}

#[cfg(feature = "animation")]
impl Default for FillMode {
    fn default() -> Self {
        FillMode::None
    }
}

#[cfg(feature = "animation")]
impl Default for Animation {
    fn default() -> Self {
        Animation {
            duration: Duration::from_secs(1),
            iterations: 1,
            direction: AnimationDirection::default(),
            fill_mode: FillMode::default(),
            delay: Duration::default(),
        }
    }
}