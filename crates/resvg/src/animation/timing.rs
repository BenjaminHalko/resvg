// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Timeline evaluation for baked animation timing.

use usvg::{Direction, TimedInterval, Timing};

/// Computes a timeline's normalized iteration progress at query time `t`.
pub(crate) fn progress(timing: &Timing, t: f32) -> Option<f32> {
    match locate(timing, t) {
        Some((interval, true)) => Some(active_progress(timing, interval, t)),
        Some((interval, false)) => interval.held(),
        None => timing.before(),
    }
}

/// Returns the contributing interval at `t`, excluding baked before values.
pub(crate) fn interval_at(timing: &Timing, t: f32) -> Option<&TimedInterval> {
    locate(timing, t).map(|(interval, _)| interval)
}

/// Returns the contributing interval's zero-based iteration at `t`.
pub(crate) fn iteration_at(timing: &Timing, t: f32) -> u32 {
    let Some((interval, active)) = locate(timing, t) else {
        return 0;
    };
    let Some(duration) = timing.iteration_dur() else {
        return 0;
    };
    let local = if active {
        t - interval.interval().begin()
    } else {
        interval_duration(interval)
    };
    let raw = local / duration;
    let floored = raw.floor();
    let completed_at_boundary = raw - floored <= f32::EPSILON && raw >= 1.0;
    let iteration = if active || !completed_at_boundary {
        floored
    } else {
        floored - 1.0
    };
    iteration.max(0.0) as u32
}

fn locate(timing: &Timing, t: f32) -> Option<(&TimedInterval, bool)> {
    let mut most_recent = None;
    for interval in timing.intervals() {
        if interval.interval().begin() <= t {
            most_recent = Some(interval);
        }
        if active(interval, t) {
            return Some((interval, true));
        }
    }
    most_recent
        .filter(|interval| interval.held().is_some())
        .map(|interval| (interval, false))
}

fn active(interval: &TimedInterval, t: f32) -> bool {
    let interval = interval.interval();
    if let Some(duration) = interval.active_duration() {
        let local = t - interval.begin();
        return local >= 0.0 && local < duration;
    }
    match interval.end() {
        Some(end) => interval.begin() <= t && t < end,
        None => interval.begin() <= t,
    }
}

fn active_progress(timing: &Timing, interval: &TimedInterval, t: f32) -> f32 {
    match timing.iteration_dur() {
        Some(duration) => directed_progress(
            (t - interval.interval().begin()) / duration,
            timing.direction(),
            false,
        ),
        None => 0.0,
    }
}

fn interval_duration(interval: &TimedInterval) -> f32 {
    let interval = interval.interval();
    interval
        .active_duration()
        .or_else(|| interval.end().map(|end| end - interval.begin()))
        .unwrap_or(0.0)
}

fn directed_progress(raw: f32, direction: Direction, at_end: bool) -> f32 {
    let (iteration, progress) = if at_end && raw > 0.0 && (raw - raw.round()).abs() <= f32::EPSILON
    {
        (raw.round() - 1.0, 1.0)
    } else {
        let iteration = raw.floor();
        (iteration, raw - iteration)
    };
    let reverse = match direction {
        Direction::Normal => false,
        Direction::Reverse => true,
        Direction::Alternate => (iteration % 2.0) >= 1.0,
        Direction::AlternateReverse => (iteration % 2.0) < 1.0,
    };
    if reverse {
        1.0 - progress
    } else {
        progress
    }
}

#[cfg(test)]
#[path = "timing_tests.rs"]
mod tests;
