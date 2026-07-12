// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Typed animation data model.
//!
//! Supports SMIL (`animate`, `animateTransform`, `animateMotion`, `set`) and CSS
//! (`@keyframes` + `animation` properties). Animations attach per-node on
//! `Group`, `Path`, and `Image`; per-gradient on paint servers; and at `Tree`
//! level for `viewBox`.
//!
//! Geometry attribute animations are baked to path-data keyframe snapshots at
//! parse time. No evaluation methods are provided; all interpolation math lives
//! in the `resvg` crate.
//!
//! The usvg writer does not serialize animations.

use std::sync::Arc;

use svgtypes::AspectRatio;

use crate::{
    FillRule, LineCap, LineJoin, NonZeroRect, NormalizedF32, Opacity, Paint, Size, StrokeMiterlimit,
    Transform,
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

/// A normalized position within a keyframe sequence.
pub type KeyOffset = NormalizedF32;

/// The easing function for a keyframe or animation.
#[derive(Clone, Copy, Debug)]
pub enum TimingFunction {
    /// Linear interpolation.
    Linear,
    /// CSS cubic-bezier easing.
    CubicBezier(f32, f32, f32, f32),
    /// CSS steps() easing.
    Steps(u32, StepPosition),
}

/// The step position for `steps()` easing.
#[derive(Clone, Copy, Debug)]
pub enum StepPosition {
    /// `jump-start` / `start`.
    JumpStart,
    /// `jump-end` / `end`.
    JumpEnd,
    /// `jump-none`.
    JumpNone,
    /// `jump-both`.
    JumpBoth,
}

/// The calculation mode for SMIL animations.
#[derive(Clone, Copy, Debug)]
pub enum CalcMode {
    /// Linear interpolation.
    Linear,
    /// Discrete stepping.
    Discrete,
    /// Paced (arc-length) interpolation.
    Paced,
    /// Cubic spline interpolation.
    Spline,
}

/// Easing parameters for a SMIL animation.
#[derive(Clone, Debug)]
pub struct Easing {
    pub(crate) calc_mode: CalcMode,
    pub(crate) key_times: Option<Vec<NormalizedF32>>,
    pub(crate) key_splines: Option<Vec<[f32; 4]>>,
}

impl Easing {
    /// Creates a new `Easing`.
    pub fn new(
        calc_mode: CalcMode,
        key_times: Option<Vec<NormalizedF32>>,
        key_splines: Option<Vec<[f32; 4]>>,
    ) -> Self {
        Self {
            calc_mode,
            key_times,
            key_splines,
        }
    }

    /// The calculation mode.
    pub fn calc_mode(&self) -> CalcMode {
        self.calc_mode
    }

    /// The key times, if specified.
    pub fn key_times(&self) -> Option<&[NormalizedF32]> {
        self.key_times.as_deref()
    }

    /// The key splines, if specified.
    pub fn key_splines(&self) -> Option<&[[f32; 4]]> {
        self.key_splines.as_deref()
    }
}

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

/// A SMIL begin/end value.
#[derive(Clone, Copy, Debug)]
pub enum Begin {
    /// A time offset in seconds.
    Offset(f32),
    /// Indefinite (never begins unless restarted).
    Indefinite,
}

/// A resolved SMIL timing interval.
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub(crate) begin: f32,
    pub(crate) end: Option<f32>,
}

impl Interval {
    /// Creates a new `Interval`.
    pub fn new(begin: f32, end: Option<f32>) -> Self {
        Self { begin, end }
    }

    /// The interval begin time in seconds.
    pub fn begin(&self) -> f32 {
        self.begin
    }

    /// The interval end time in seconds, or `None` if open/indefinite.
    pub fn end(&self) -> Option<f32> {
        self.end
    }
}

/// The simple duration of a SMIL animation.
#[derive(Clone, Copy, Debug)]
pub enum Dur {
    /// A finite duration in seconds.
    Seconds(f32),
    /// Indefinite duration.
    Indefinite,
}

/// The repeat count of a SMIL animation.
#[derive(Clone, Copy, Debug)]
pub enum RepeatCount {
    /// A finite repeat count.
    Count(f32),
    /// Repeat indefinitely.
    Indefinite,
}

/// The fill behavior of a SMIL animation.
#[derive(Clone, Copy, Debug)]
pub enum SmilFill {
    /// Hold the final value after the animation ends.
    Freeze,
    /// Remove the animation effect after it ends.
    Remove,
}

/// The restart behavior of a SMIL animation.
#[derive(Clone, Copy, Debug)]
pub enum Restart {
    /// Always restart.
    Always,
    /// Never restart.
    Never,
    /// Restart only when not active.
    WhenNotActive,
}

/// SMIL animation timing.
#[derive(Clone, Debug)]
pub struct SmilTiming {
    pub(crate) begins: Vec<Begin>,
    pub(crate) dur: Dur,
    pub(crate) ends: Vec<Begin>,
    pub(crate) repeat_count: Option<RepeatCount>,
    pub(crate) repeat_dur: Option<f32>,
    pub(crate) fill: SmilFill,
    pub(crate) restart: Restart,
    pub(crate) intervals: Vec<Interval>,
}

impl SmilTiming {
    /// Creates a new `SmilTiming`.
    pub fn new(
        begins: Vec<Begin>,
        dur: Dur,
        ends: Vec<Begin>,
        repeat_count: Option<RepeatCount>,
        repeat_dur: Option<f32>,
        fill: SmilFill,
        restart: Restart,
        intervals: Vec<Interval>,
    ) -> Self {
        Self {
            begins,
            dur,
            ends,
            repeat_count,
            repeat_dur,
            fill,
            restart,
            intervals,
        }
    }

    /// The begin values.
    pub fn begins(&self) -> &[Begin] {
        &self.begins
    }

    /// The simple duration.
    pub fn dur(&self) -> &Dur {
        &self.dur
    }

    /// The end values.
    pub fn ends(&self) -> &[Begin] {
        &self.ends
    }

    /// The repeat count, if specified.
    pub fn repeat_count(&self) -> Option<&RepeatCount> {
        self.repeat_count.as_ref()
    }

    /// The repeat duration in seconds, if specified.
    pub fn repeat_dur(&self) -> Option<f32> {
        self.repeat_dur
    }

    /// The fill behavior.
    pub fn fill(&self) -> SmilFill {
        self.fill
    }

    /// The restart behavior.
    pub fn restart(&self) -> Restart {
        self.restart
    }

    /// The resolved timing intervals.
    pub fn intervals(&self) -> &[Interval] {
        &self.intervals
    }
}

/// The iteration count of a CSS animation.
#[derive(Clone, Copy, Debug)]
pub enum Iterations {
    /// A finite count.
    Count(f32),
    /// Infinite iterations.
    Infinite,
}

/// The direction of a CSS animation.
#[derive(Clone, Copy, Debug)]
pub enum Direction {
    /// Normal direction.
    Normal,
    /// Reverse direction.
    Reverse,
    /// Alternate direction.
    Alternate,
    /// Alternate-reverse direction.
    AlternateReverse,
}

/// The fill mode of a CSS animation.
#[derive(Clone, Copy, Debug)]
pub enum CssFillMode {
    /// No fill.
    None,
    /// Hold the final value after the animation ends.
    Forwards,
    /// Apply the first keyframe before the animation starts.
    Backwards,
    /// Both forwards and backwards.
    Both,
}

/// The play state of a CSS animation.
#[derive(Clone, Copy, Debug)]
pub enum PlayState {
    /// The animation is running.
    Running,
    /// The animation is paused.
    Paused,
}

/// CSS animation timing.
#[derive(Clone, Copy, Debug)]
pub struct CssTiming {
    pub(crate) duration: f32,
    pub(crate) delay: f32,
    pub(crate) iterations: Iterations,
    pub(crate) direction: Direction,
    pub(crate) fill_mode: CssFillMode,
    pub(crate) timing_function: TimingFunction,
    pub(crate) play_state: PlayState,
}

impl CssTiming {
    /// Creates a new `CssTiming`.
    pub fn new(
        duration: f32,
        delay: f32,
        iterations: Iterations,
        direction: Direction,
        fill_mode: CssFillMode,
        timing_function: TimingFunction,
        play_state: PlayState,
    ) -> Self {
        Self {
            duration,
            delay,
            iterations,
            direction,
            fill_mode,
            timing_function,
            play_state,
        }
    }

    /// The animation duration in seconds.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// The animation delay in seconds (may be negative).
    pub fn delay(&self) -> f32 {
        self.delay
    }

    /// The iteration count.
    pub fn iterations(&self) -> &Iterations {
        &self.iterations
    }

    /// The animation direction.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// The fill mode.
    pub fn fill_mode(&self) -> CssFillMode {
        self.fill_mode
    }

    /// The timing function.
    pub fn timing_function(&self) -> &TimingFunction {
        &self.timing_function
    }

    /// The play state.
    pub fn play_state(&self) -> PlayState {
        self.play_state
    }
}

/// The timing of an animation — SMIL or CSS.
#[derive(Clone, Debug)]
pub enum Timing {
    /// SMIL timing.
    Smil(SmilTiming),
    /// CSS timing.
    Css(CssTiming),
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
}

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
    pub(crate) source_stops: Vec<SourceStop>,
    pub(crate) source_indices: Vec<Option<usize>>,
}

impl GradientAnimation {
    /// Creates a new `GradientAnimation`.
    pub fn new(
        animations: Vec<Arc<Animation>>,
        underlying_r: Option<f32>,
        source_stops: Vec<SourceStop>,
        source_indices: Vec<Option<usize>>,
    ) -> Self {
        Self {
            animations,
            underlying_r,
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

    /// The source stops.
    pub fn source_stops(&self) -> &[SourceStop] {
        &self.source_stops
    }

    /// Returns the source stop index for a given converted stop index.
    pub fn source_index_of(&self, stop_index: usize) -> Option<usize> {
        self.source_indices.get(stop_index).copied().flatten()
    }
}

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
        super::geom::ViewBox {
            rect: sampled_rect,
            aspect: self.static_aspect,
        }
        .to_transform(tree_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_animation_kinds() {
        let mut pb = tiny_skia_path::PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(1.0, 1.0);
        let path = Arc::new(pb.finish().unwrap());

        let _ = AnimationKind::Transform(TransformTrack::Smil {
            kind: TransformKind::Translate,
            keyframes: vec![],
        });
        let _ = AnimationKind::GradientTransform(TransformTrack::Css {
            keyframes: vec![],
            origin: TransformOrigin::new(
                TransformOriginValue::Length(0.0),
                TransformOriginValue::Percent(50.0),
            ),
            box_: TransformBox::ViewBox,
        });
        let _ = AnimationKind::Motion(MotionTrack::new(path, None, MotionRotate::Auto));
        let _ = AnimationKind::Opacity(Track::new(vec![]));
        let _ = AnimationKind::Fill(Track::new(vec![]));
        let _ = AnimationKind::Stroke(Track::new(vec![]));
        let _ = AnimationKind::StrokeWidth(Track::new(vec![]));
        let _ = AnimationKind::StrokeDashoffset(Track::new(vec![]));
        let _ = AnimationKind::StrokeDasharray(Track::new(vec![]));
        let _ = AnimationKind::StrokeMiterlimit(Track::new(vec![]));
        let _ = AnimationKind::StrokeLinecap(Track::new(vec![]));
        let _ = AnimationKind::StrokeLinejoin(Track::new(vec![]));
        let _ = AnimationKind::FillRule(Track::new(vec![]));
        let _ = AnimationKind::Display(Track::new(vec![]));
        let _ = AnimationKind::Visibility(Track::new(vec![]));
        let _ = AnimationKind::Path(PathTrack::new(vec![], None));
        let _ = AnimationKind::StopColor(Track::new(vec![]));
        let _ = AnimationKind::StopOpacity(Track::new(vec![]));
        let _ = AnimationKind::StopOffset(Track::new(vec![]));
        let _ = AnimationKind::GradientGeometry(Track::new(vec![]));
        let _ = AnimationKind::ViewBox(Track::new(vec![]));
        let _ = AnimationKind::ImageX(Track::new(vec![]));
        let _ = AnimationKind::ImageY(Track::new(vec![]));
        let _ = AnimationKind::ImageWidth(Track::new(vec![]));
        let _ = AnimationKind::ImageHeight(Track::new(vec![]));
    }
}
