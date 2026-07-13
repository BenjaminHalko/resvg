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

/// Easing parameters for an animation.
#[derive(Clone, Debug)]
pub struct Easing {
    pub(crate) calc_mode: CalcMode,
    pub(crate) key_times: Option<Vec<NormalizedF32>>,
    pub(crate) key_splines: Option<Vec<[f32; 4]>>,
    pub(crate) timing_function: Option<TimingFunction>,
}

impl Easing {
    /// Creates easing without an animation-level timing function.
    pub fn new(
        calc_mode: CalcMode,
        key_times: Option<Vec<NormalizedF32>>,
        key_splines: Option<Vec<[f32; 4]>>,
    ) -> Self {
        Self {
            calc_mode,
            key_times,
            key_splines,
            timing_function: None,
        }
    }

    /// Adds an animation-level timing function.
    pub fn with_timing_function(mut self, timing_function: TimingFunction) -> Self {
        self.timing_function = Some(timing_function);
        self
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

    /// The animation-level timing function, if specified.
    pub fn timing_function(&self) -> Option<&TimingFunction> {
        self.timing_function.as_ref()
    }
}

/// A resolved animation interval.
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub(crate) begin: f32,
    pub(crate) end: Option<f32>,
    pub(crate) active_duration: Option<f32>,
}

impl Interval {
    /// Creates an interval with an absolute end time.
    pub fn new(begin: f32, end: Option<f32>) -> Self {
        Self {
            begin,
            end,
            active_duration: None,
        }
    }

    /// Creates a finite interval whose end is represented relative to its begin.
    ///
    /// This retains a short active duration even when adding it to a large begin
    /// would lose precision in `f32`.
    pub fn new_relative(begin: f32, active_duration: f32) -> Self {
        Self {
            begin,
            end: Some(begin + active_duration),
            active_duration: Some(active_duration),
        }
    }

    /// The interval begin time in seconds.
    pub fn begin(&self) -> f32 {
        self.begin
    }

    /// The interval end time in seconds, or `None` if open/indefinite.
    pub fn end(&self) -> Option<f32> {
        self.end
    }

    /// The active duration represented relative to the begin, if any.
    pub fn active_duration(&self) -> Option<f32> {
        self.active_duration
    }
}

/// The direction applied to each iteration.
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

/// A resolved interval with the baked value it holds after becoming inactive.
#[derive(Clone, Copy, Debug)]
pub struct TimedInterval {
    pub(crate) interval: Interval,
    pub(crate) held: Option<f32>,
}

impl TimedInterval {
    /// Creates an interval with its baked held value.
    pub fn new(interval: Interval, held: Option<f32>) -> Self {
        Self { interval, held }
    }

    /// The resolved interval.
    pub fn interval(&self) -> &Interval {
        &self.interval
    }

    /// The value held after the interval, if any.
    pub fn held(&self) -> Option<f32> {
        self.held
    }
}

/// The fully resolved timeline of an animation.
#[derive(Clone, Debug)]
pub struct Timing {
    pub(crate) intervals: Vec<TimedInterval>,
    pub(crate) iteration_dur: Option<f32>,
    pub(crate) direction: Direction,
    pub(crate) before: Option<f32>,
    pub(crate) one_loop_end: Option<f32>,
}

impl Timing {
    /// Creates a fully resolved animation timeline.
    pub fn new(
        intervals: Vec<TimedInterval>,
        iteration_dur: Option<f32>,
        direction: Direction,
        before: Option<f32>,
        one_loop_end: Option<f32>,
    ) -> Self {
        Self {
            intervals,
            iteration_dur,
            direction,
            before,
            one_loop_end,
        }
    }

    /// The resolved animation intervals.
    pub fn intervals(&self) -> &[TimedInterval] {
        &self.intervals
    }

    /// The duration of one iteration, if it is finite and non-zero.
    pub fn iteration_dur(&self) -> Option<f32> {
        self.iteration_dur
    }

    /// The direction applied to each iteration.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// The value applied before the first interval, if any.
    pub fn before(&self) -> Option<f32> {
        self.before
    }

    /// The independently resolved end time of the first loop.
    pub fn one_loop_end(&self) -> Option<f32> {
        self.one_loop_end
    }
}
