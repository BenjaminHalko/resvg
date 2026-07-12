// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::NormalizedF32;

use super::TimingFunction;

/// A single keyframe value.
#[derive(Clone, Debug)]
pub struct Keyframe<T> {
    pub(crate) offset: NormalizedF32,
    pub(crate) value: T,
    pub(crate) timing_function: Option<TimingFunction>,
}

impl<T: Clone> Keyframe<T> {
    /// Creates a new `Keyframe`.
    pub fn new(offset: NormalizedF32, value: T, timing_function: Option<TimingFunction>) -> Self {
        Self {
            offset,
            value,
            timing_function,
        }
    }

    /// The keyframe offset in [0, 1].
    pub fn offset(&self) -> NormalizedF32 {
        self.offset
    }

    /// The keyframe value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// The per-keyframe timing function, if any.
    pub fn timing_function(&self) -> Option<&TimingFunction> {
        self.timing_function.as_ref()
    }
}

/// A sequence of keyframes.
#[derive(Clone, Debug)]
pub struct Track<T> {
    pub(crate) keyframes: Vec<Keyframe<T>>,
}

impl<T: Clone> Track<T> {
    /// Creates a new `Track`.
    pub fn new(keyframes: Vec<Keyframe<T>>) -> Self {
        Self { keyframes }
    }

    /// The keyframes.
    pub fn keyframes(&self) -> &[Keyframe<T>] {
        &self.keyframes
    }
}

/// A SMIL transform track.
#[derive(Clone, Debug)]
pub enum TransformTrack {
    /// A SMIL transform animation with typed parameters.
    Smil {
        /// The transform kind.
        kind: TransformKind,
        /// The keyframes (each value is a parameter list).
        keyframes: Vec<Keyframe<Vec<f32>>>,
    },
    /// A CSS transform animation.
    Css {
        /// The keyframes (each value is a list of transform functions).
        keyframes: Vec<Keyframe<Vec<TransformFunction>>>,
        /// The transform origin.
        origin: TransformOrigin,
        /// The transform box.
        box_: TransformBox,
    },
}

/// The kind of a SMIL transform animation.
#[derive(Clone, Copy, Debug)]
pub enum TransformKind {
    /// `translate(tx [ty])`.
    Translate,
    /// `scale(sx [sy])`.
    Scale,
    /// `rotate(angle [cx cy])`.
    Rotate,
    /// `skewX(angle)`.
    SkewX,
    /// `skewY(angle)`.
    SkewY,
}

/// A CSS transform function.
#[derive(Clone, Copy, Debug)]
pub enum TransformFunction {
    /// `matrix(a b c d e f)`.
    Matrix(f32, f32, f32, f32, f32, f32),
    /// `translate(tx [ty])`.
    Translate(f32, f32),
    /// `translateX(tx)`.
    TranslateX(f32),
    /// `translateY(ty)`.
    TranslateY(f32),
    /// `scale(sx [sy])`.
    Scale(f32, f32),
    /// `scaleX(sx)`.
    ScaleX(f32),
    /// `scaleY(sy)`.
    ScaleY(f32),
    /// `rotate(angle)`.
    Rotate(f32),
    /// `skewX(angle)`.
    SkewX(f32),
    /// `skewY(angle)`.
    SkewY(f32),
}

/// The transform origin for CSS animations.
#[derive(Clone, Copy, Debug)]
pub struct TransformOrigin {
    pub(crate) x: TransformOriginValue,
    pub(crate) y: TransformOriginValue,
}

impl TransformOrigin {
    /// Creates a new `TransformOrigin`.
    pub fn new(x: TransformOriginValue, y: TransformOriginValue) -> Self {
        Self { x, y }
    }

    /// The x component.
    pub fn x(&self) -> &TransformOriginValue {
        &self.x
    }

    /// The y component.
    pub fn y(&self) -> &TransformOriginValue {
        &self.y
    }
}

/// A single component of a transform origin.
#[derive(Clone, Copy, Debug)]
pub enum TransformOriginValue {
    /// An absolute length in user units.
    Length(f32),
    /// A percentage of the reference box.
    Percent(f32),
}

/// The transform box for CSS animations.
#[derive(Clone, Copy, Debug)]
pub enum TransformBox {
    /// `content-box`.
    ContentBox,
    /// `border-box`.
    BorderBox,
    /// `fill-box`.
    FillBox,
    /// `stroke-box`.
    StrokeBox,
    /// `view-box`.
    ViewBox,
}

/// A motion animation track.
#[derive(Clone, Debug)]
pub struct MotionTrack {
    pub(crate) path: Arc<tiny_skia_path::Path>,
    pub(crate) key_points: Option<Vec<NormalizedF32>>,
    pub(crate) rotate: MotionRotate,
}

impl MotionTrack {
    /// Creates a new `MotionTrack`.
    pub fn new(
        path: Arc<tiny_skia_path::Path>,
        key_points: Option<Vec<NormalizedF32>>,
        rotate: MotionRotate,
    ) -> Self {
        Self {
            path,
            key_points,
            rotate,
        }
    }

    /// The motion path.
    pub fn path(&self) -> &tiny_skia_path::Path {
        &self.path
    }

    /// The key points, if specified.
    pub fn key_points(&self) -> Option<&[NormalizedF32]> {
        self.key_points.as_deref()
    }

    /// The rotation mode.
    pub fn rotate(&self) -> MotionRotate {
        self.rotate
    }
}

/// The rotation mode for motion animations.
#[derive(Clone, Copy, Debug)]
pub enum MotionRotate {
    /// Rotate automatically to follow the path tangent.
    Auto,
    /// Rotate automatically, reversed.
    AutoReverse,
    /// A fixed angle in degrees.
    Angle(f32),
}

/// A single keyframe in a path animation.
#[derive(Clone, Debug)]
pub struct PathKeyframe {
    pub(crate) offset: NormalizedF32,
    pub(crate) path: Arc<tiny_skia_path::Path>,
    pub(crate) renderable: bool,
    pub(crate) timing_function: Option<TimingFunction>,
}

impl PathKeyframe {
    /// Creates a new `PathKeyframe`.
    pub fn new(
        offset: NormalizedF32,
        path: Arc<tiny_skia_path::Path>,
        renderable: bool,
        timing_function: Option<TimingFunction>,
    ) -> Self {
        Self {
            offset,
            path,
            renderable,
            timing_function,
        }
    }

    /// The keyframe offset in [0, 1].
    pub fn offset(&self) -> NormalizedF32 {
        self.offset
    }

    /// The baked path at this keyframe.
    pub fn path(&self) -> &tiny_skia_path::Path {
        &self.path
    }

    /// Whether this keyframe produces a renderable shape.
    pub fn renderable(&self) -> bool {
        self.renderable
    }

    /// The per-keyframe timing function, if any.
    pub fn timing_function(&self) -> Option<&TimingFunction> {
        self.timing_function.as_ref()
    }
}

/// A baked path animation track.
#[derive(Clone, Debug)]
pub struct PathTrack {
    pub(crate) keyframes: Vec<PathKeyframe>,
    pub(crate) accumulation_delta: Option<Arc<tiny_skia_path::Path>>,
    pub(crate) replaces_geometry: bool,
}

impl PathTrack {
    /// Creates a new `PathTrack`.
    pub fn new(
        keyframes: Vec<PathKeyframe>,
        accumulation_delta: Option<Arc<tiny_skia_path::Path>>,
    ) -> Self {
        Self {
            keyframes,
            accumulation_delta,
            replaces_geometry: false,
        }
    }

    /// Creates a path track that replaces every prior geometry contribution.
    pub(crate) fn new_replacing_geometry(
        keyframes: Vec<PathKeyframe>,
        accumulation_delta: Option<Arc<tiny_skia_path::Path>>,
    ) -> Self {
        Self {
            keyframes,
            accumulation_delta,
            replaces_geometry: true,
        }
    }

    /// The keyframes.
    pub fn keyframes(&self) -> &[PathKeyframe] {
        &self.keyframes
    }

    /// The accumulation delta path, if `accumulate=sum` was specified.
    pub fn accumulation_delta(&self) -> Option<&tiny_skia_path::Path> {
        self.accumulation_delta.as_deref()
    }

    /// Whether this track replaces all prior geometry contributions.
    pub fn replaces_geometry(&self) -> bool {
        self.replaces_geometry
    }
}
