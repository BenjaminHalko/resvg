// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Timeline evaluation for animation timing.
//!
//! Turns a query time into a normalized iteration progress in `[0, 1]` for both
//! SMIL (`SmilTiming`, consuming the intervals resolved at parse time) and CSS
//! (`CssTiming`, applying delay, iteration counting, direction, fill-mode, and
//! play-state).

use usvg::{
    CssFillMode, CssTiming, Direction, Dur, Interval, Iterations, PlayState, SmilFill, SmilTiming,
};

/// Computes the SMIL iteration progress at query time `t`, in seconds.
///
/// The resolved `intervals` (produced by the parser) are consumed verbatim; the
/// `begin`/`end`/`restart` fields have already been folded into them. Returns
/// `None` when the animation contributes nothing at `t` (before it starts, or in
/// a gap/after the end with `fill=remove`).
pub(crate) fn smil_progress(timing: &SmilTiming, t: f32) -> Option<f32> {
    let intervals = timing.intervals();

    // The most recent interval whose begin has been reached. Intervals are
    // sorted ascending by begin and are non-overlapping, so the last match is
    // the relevant one for freeze/gap handling.
    let mut most_recent: Option<&Interval> = None;

    for interval in intervals {
        if interval.begin() <= t {
            most_recent = Some(interval);
        }

        let active = match interval.end() {
            Some(end) => interval.begin() <= t && t < end,
            // An open interval stays active for all times at or after its begin.
            None => interval.begin() <= t,
        };

        if active {
            return Some(active_progress(interval.begin(), t, timing.dur()));
        }
    }

    // `t` is not inside any interval: before the first, in a gap, or past the end.
    match most_recent {
        // Before the first interval begins the animation contributes nothing.
        None => None,
        Some(interval) => match timing.fill() {
            SmilFill::Freeze => Some(frozen_progress(interval, timing.dur())),
            SmilFill::Remove => None,
        },
    }
}

/// The progress within an active interval at time `t`.
fn active_progress(begin: f32, t: f32, dur: &Dur) -> f32 {
    let local = t - begin;
    match *dur {
        Dur::Seconds(seconds) if seconds > 0.0 => {
            // Repeating iterations wrap: a boundary hit mid-animation restarts
            // the next iteration at progress 0.
            (local / seconds).fract()
        }
        // A zero or indefinite simple duration holds at the start value.
        _ => 0.0,
    }
}

/// The frozen progress held after an interval ends under `fill=freeze`.
///
/// The held value is the progress at the interval's actual resolved end (which
/// already reflects `repeatCount`/`repeatDur` truncation and any clipping),
/// not the nominal active-duration endpoint. A boundary landing exactly on an
/// iteration edge freezes at the end value (`1.0`) rather than wrapping to `0.0`.
fn frozen_progress(interval: &Interval, dur: &Dur) -> f32 {
    let Some(end) = interval.end() else {
        // Open intervals never end, so this is unreachable in practice.
        return 0.0;
    };

    let local = end - interval.begin();
    match *dur {
        Dur::Seconds(seconds) if seconds > 0.0 => {
            let raw = local / seconds;
            let floored = raw.floor();
            let frac = raw - floored;
            if frac <= f32::EPSILON && raw >= 1.0 {
                1.0
            } else {
                frac
            }
        }
        _ => 1.0,
    }
}

/// Computes the CSS iteration progress at query time `t`, in seconds.
///
/// Applies `delay` (including negative delays), fractional/infinite iteration
/// counts, `direction`, `fill-mode`, and `play-state`. Returns `None` when the
/// animation contributes nothing at `t` under the active fill-mode.
pub(crate) fn css_progress(timing: &CssTiming, t: f32) -> Option<f32> {
    let dur = timing.duration();
    let delay = timing.delay();
    let direction = timing.direction();
    let fill = timing.fill_mode();

    let iter_count = match *timing.iterations() {
        Iterations::Count(count) => count.max(0.0),
        Iterations::Infinite => f32::INFINITY,
    };

    // While paused, the animation holds the progress it had in the initial
    // style at t=0. A negative delay advances that starting point.
    let local_time = match timing.play_state() {
        PlayState::Paused => (-delay).max(0.0),
        PlayState::Running => t - delay,
    };

    // Before the first iteration starts (positive delay not yet elapsed).
    if local_time < 0.0 {
        return match fill {
            CssFillMode::Backwards | CssFillMode::Both => {
                Some(directed_progress(0.0, direction, false))
            }
            CssFillMode::None | CssFillMode::Forwards => None,
        };
    }

    let active_dur = dur * iter_count;

    // After the whole animation ends (only reachable with finite durations).
    if dur > 0.0 && active_dur.is_finite() && local_time >= active_dur {
        return match fill {
            CssFillMode::Forwards | CssFillMode::Both => {
                Some(directed_progress(iter_count, direction, true))
            }
            CssFillMode::None | CssFillMode::Backwards => None,
        };
    }

    if dur <= 0.0 {
        // A zero-duration animation is instantaneous: it is already at its end.
        return match fill {
            CssFillMode::Forwards | CssFillMode::Both => {
                Some(directed_progress(iter_count, direction, true))
            }
            CssFillMode::None | CssFillMode::Backwards => None,
        };
    }

    let raw = local_time / dur;
    Some(directed_progress(raw, direction, false))
}

/// Maps an overall iteration position `raw` to a direction-adjusted progress.
///
/// When `at_end` is set and `raw` lands exactly on an iteration boundary, the
/// final iteration is held at progress `1.0` rather than wrapping to `0.0` of
/// the next iteration.
fn directed_progress(raw: f32, direction: Direction, at_end: bool) -> f32 {
    let (iteration, iter_progress) = if at_end && raw > 0.0 && (raw - raw.round()).abs() <= f32::EPSILON
    {
        (raw.round() - 1.0, 1.0)
    } else {
        let floored = raw.floor();
        (floored, raw - floored)
    };

    let odd = (iteration % 2.0) >= 1.0;
    let reverse = match direction {
        Direction::Normal => false,
        Direction::Reverse => true,
        Direction::Alternate => odd,
        Direction::AlternateReverse => !odd,
    };

    if reverse {
        1.0 - iter_progress
    } else {
        iter_progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usvg::{Begin, RepeatCount, Restart, TimingFunction};

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "expected {b}, got {a}");
    }

    fn smil(dur: Dur, fill: SmilFill, intervals: Vec<Interval>) -> SmilTiming {
        SmilTiming::new(
            vec![Begin::Offset(0.0)],
            dur,
            vec![],
            None,
            None,
            fill,
            Restart::Always,
            intervals,
        )
    }

    // --- SMIL ---------------------------------------------------------------

    #[test]
    fn smil_before_start_is_none() {
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Remove,
            vec![Interval::new(1.0, Some(2.0))],
        );
        assert_eq!(smil_progress(&timing, 0.5), None);
    }

    #[test]
    fn smil_active_progress() {
        let timing = smil(
            Dur::Seconds(2.0),
            SmilFill::Remove,
            vec![Interval::new(0.0, Some(2.0))],
        );
        approx(smil_progress(&timing, 0.0).unwrap(), 0.0);
        approx(smil_progress(&timing, 0.5).unwrap(), 0.25);
        approx(smil_progress(&timing, 1.0).unwrap(), 0.5);
    }

    #[test]
    fn smil_remove_after_end_is_none() {
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Remove,
            vec![Interval::new(0.0, Some(1.0))],
        );
        assert_eq!(smil_progress(&timing, 2.0), None);
    }

    #[test]
    fn smil_freeze_fractional_repeat_holds_mid_iteration() {
        // repeatCount=2.5, dur=1s => active end 2.5; frozen at fract(2.5) = 0.5.
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Freeze,
            vec![Interval::new(0.0, Some(2.5))],
        );
        approx(smil_progress(&timing, 3.0).unwrap(), 0.5);
    }

    #[test]
    fn smil_freeze_integer_repeat_holds_end_value() {
        // repeatCount=2, dur=1s => active end 2.0; boundary freezes at 1.0.
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Freeze,
            vec![Interval::new(0.0, Some(2.0))],
        );
        approx(smil_progress(&timing, 5.0).unwrap(), 1.0);
    }

    #[test]
    fn smil_freeze_repeat_dur_truncates_mid_iteration() {
        // dur=1s, repeatDur=2.5s => end 2.5; frozen at 0.5.
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Freeze,
            vec![Interval::new(0.0, Some(2.5))],
        );
        approx(smil_progress(&timing, 10.0).unwrap(), 0.5);
    }

    #[test]
    fn smil_freeze_clipped_end_uses_actual_end() {
        // dur=10s but the interval is clipped at t=3 by an end instant.
        // Frozen progress uses the actual end: 3/10 = 0.3.
        let timing = smil(
            Dur::Seconds(10.0),
            SmilFill::Freeze,
            vec![Interval::new(0.0, Some(3.0))],
        );
        approx(smil_progress(&timing, 5.0).unwrap(), 0.3);
    }

    #[test]
    fn smil_freeze_across_gap() {
        // begin="0s;3s" dur="1s" => intervals [0,1] and [3,4].
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Freeze,
            vec![Interval::new(0.0, Some(1.0)), Interval::new(3.0, Some(4.0))],
        );
        // t=2 is in the gap: hold the first interval's endpoint (1.0).
        approx(smil_progress(&timing, 2.0).unwrap(), 1.0);
        // t=3 the second interval takes over at progress 0.
        approx(smil_progress(&timing, 3.0).unwrap(), 0.0);
    }

    #[test]
    fn smil_multi_interval_picks_correct_interval() {
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Remove,
            vec![Interval::new(0.0, Some(1.0)), Interval::new(3.0, Some(4.0))],
        );
        approx(smil_progress(&timing, 3.5).unwrap(), 0.5);
    }

    #[test]
    fn smil_restart_never_ignores_later_begins() {
        // restart=never leaves only the first interval resolved even though the
        // begin list carries a second entry. The consumer reads intervals, not
        // begins, so the later begin never restarts the animation.
        let timing = SmilTiming::new(
            vec![Begin::Offset(0.0), Begin::Offset(3.0)],
            Dur::Seconds(1.0),
            vec![],
            Some(RepeatCount::Count(1.0)),
            None,
            SmilFill::Remove,
            Restart::Never,
            vec![Interval::new(0.0, Some(1.0))],
        );
        // At t=3.5 (where a second begin would live) nothing is contributed.
        assert_eq!(smil_progress(&timing, 3.5), None);
    }

    #[test]
    fn smil_open_interval_stays_active() {
        let timing = smil(
            Dur::Seconds(1.0),
            SmilFill::Remove,
            vec![Interval::new(0.0, None)],
        );
        approx(smil_progress(&timing, 5.5).unwrap(), 0.5);
    }

    // --- CSS ----------------------------------------------------------------

    fn css(
        duration: f32,
        delay: f32,
        iterations: Iterations,
        direction: Direction,
        fill_mode: CssFillMode,
        play_state: PlayState,
    ) -> CssTiming {
        CssTiming::new(
            duration,
            delay,
            iterations,
            direction,
            fill_mode,
            TimingFunction::Linear,
            play_state,
        )
    }

    #[test]
    fn css_basic_active_progress() {
        let timing = css(
            2.0,
            0.0,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::None,
            PlayState::Running,
        );
        approx(css_progress(&timing, 1.0).unwrap(), 0.5);
    }

    #[test]
    fn css_positive_delay_before_start() {
        let timing = css(
            1.0,
            1.0,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::None,
            PlayState::Running,
        );
        assert_eq!(css_progress(&timing, 0.5), None);
    }

    #[test]
    fn css_backwards_fill_before_start() {
        let timing = css(
            1.0,
            1.0,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::Backwards,
            PlayState::Running,
        );
        approx(css_progress(&timing, 0.5).unwrap(), 0.0);
    }

    #[test]
    fn css_negative_delay_mid_iteration_start() {
        // delay=-0.5, dur=1 => at t=0 the animation is already 0.5 through.
        let timing = css(
            1.0,
            -0.5,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::None,
            PlayState::Running,
        );
        approx(css_progress(&timing, 0.0).unwrap(), 0.5);
    }

    #[test]
    fn css_forwards_fill_after_end() {
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::Forwards,
            PlayState::Running,
        );
        approx(css_progress(&timing, 5.0).unwrap(), 1.0);
    }

    #[test]
    fn css_no_fill_after_end_is_none() {
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::None,
            PlayState::Running,
        );
        assert_eq!(css_progress(&timing, 5.0), None);
    }

    #[test]
    fn css_fractional_iterations_forwards_end() {
        // 2.5 iterations: the end freezes at fract(2.5) = 0.5 of the last (even) iteration.
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(2.5),
            Direction::Normal,
            CssFillMode::Forwards,
            PlayState::Running,
        );
        approx(css_progress(&timing, 10.0).unwrap(), 0.5);
    }

    #[test]
    fn css_infinite_iterations_wraps() {
        let timing = css(
            1.0,
            0.0,
            Iterations::Infinite,
            Direction::Normal,
            CssFillMode::None,
            PlayState::Running,
        );
        approx(css_progress(&timing, 3.25).unwrap(), 0.25);
    }

    #[test]
    fn css_reverse_direction() {
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(1.0),
            Direction::Reverse,
            CssFillMode::None,
            PlayState::Running,
        );
        approx(css_progress(&timing, 0.25).unwrap(), 0.75);
    }

    #[test]
    fn css_alternate_second_iteration_mirrored() {
        // Alternate: iteration index 1 (odd) is reversed.
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(3.0),
            Direction::Alternate,
            CssFillMode::None,
            PlayState::Running,
        );
        // Iteration 0 at t=0.25 => 0.25.
        approx(css_progress(&timing, 0.25).unwrap(), 0.25);
        // Iteration 1 at t=1.25 => reversed => 0.75.
        approx(css_progress(&timing, 1.25).unwrap(), 0.75);
    }

    #[test]
    fn css_alternate_reverse_iteration_two_mirrored() {
        // Alternate-reverse: even iteration indices (0, 2, ...) are reversed.
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(4.0),
            Direction::AlternateReverse,
            CssFillMode::None,
            PlayState::Running,
        );
        // Iteration index 2 (the third iteration) at t=2.25 => reversed => 0.75.
        approx(css_progress(&timing, 2.25).unwrap(), 0.75);
        // Iteration index 1 (odd) at t=1.25 => normal => 0.25.
        approx(css_progress(&timing, 1.25).unwrap(), 0.25);
    }

    #[test]
    fn css_paused_with_negative_delay_holds_advanced_progress() {
        // Paused holds the initial-style progress; a negative delay advances it.
        let timing = css(
            1.0,
            -0.5,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::None,
            PlayState::Paused,
        );
        // Regardless of the query time, the paused progress is held at 0.5.
        approx(css_progress(&timing, 0.0).unwrap(), 0.5);
        approx(css_progress(&timing, 10.0).unwrap(), 0.5);
    }

    #[test]
    fn css_paused_without_delay_holds_start() {
        let timing = css(
            1.0,
            0.0,
            Iterations::Count(1.0),
            Direction::Normal,
            CssFillMode::None,
            PlayState::Paused,
        );
        approx(css_progress(&timing, 5.0).unwrap(), 0.0);
    }
}
