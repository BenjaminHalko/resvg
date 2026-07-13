use super::*;
use usvg::{Direction, Interval, TimedInterval, Timing};

fn approx(actual: Option<f32>, expected: Option<f32>) {
    match (actual, expected) {
        (Some(actual), Some(expected)) => {
            assert!(
                (actual - expected).abs() <= 1e-6,
                "expected {expected}, got {actual}"
            );
        }
        (None, None) => {}
        (actual, expected) => panic!("expected {expected:?}, got {actual:?}"),
    }
}

fn timeline(
    intervals: Vec<TimedInterval>,
    iteration_dur: Option<f32>,
    direction: Direction,
    before: Option<f32>,
    one_loop_end: Option<f32>,
) -> Timing {
    Timing::new(intervals, iteration_dur, direction, before, one_loop_end)
}

fn absolute(begin: f32, end: Option<f32>, held: Option<f32>) -> TimedInterval {
    TimedInterval::new(Interval::new(begin, end), held)
}

fn relative(begin: f32, duration: f32, held: Option<f32>) -> TimedInterval {
    TimedInterval::new(Interval::new_relative(begin, duration), held)
}

#[path = "timing_tests/css.rs"]
mod css;
#[path = "timing_tests/smil.rs"]
mod smil;
