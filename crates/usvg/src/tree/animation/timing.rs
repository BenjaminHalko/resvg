// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::NormalizedF32;

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

impl Timing {
    /// The end time of the first loop, in seconds, or `None` when the simple
    /// duration is indefinite. Repeats collapse to a single loop.
    pub(crate) fn one_loop_end(&self) -> Option<f32> {
        match self {
            Timing::Smil(smil) => {
                let simple = match smil.dur {
                    Dur::Seconds(seconds) if seconds > 0.0 => seconds,
                    _ => return None,
                };
                let begin = smil
                    .intervals
                    .iter()
                    .map(|interval| interval.begin)
                    .fold(f32::INFINITY, f32::min);
                begin.is_finite().then_some(begin + simple)
            }
            Timing::Css(css) => Some(css.delay.max(0.0) + css.duration),
        }
    }
}
