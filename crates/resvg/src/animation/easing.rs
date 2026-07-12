// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Easing functions for animation timing.
//!
//! Implements CSS `cubic-bezier()` (Newton's method with a bisection fallback),
//! CSS `steps()` with all four jump variants, SMIL `keySplines` segment
//! evaluation, and the keyword easing beziers (`ease`, `ease-in`, ...).

use usvg::{StepPosition, TimingFunction};

/// Evaluates a CSS `cubic-bezier(x1, y1, x2, y2)` easing at `t`.
///
/// The control polygon is `P0 = (0, 0)`, `P1 = (x1, y1)`, `P2 = (x2, y2)`,
/// `P3 = (1, 1)`. Given the input progress `t` on the x axis, the parametric
/// `s` with `X(s) = t` is found via Newton's method with a bisection fallback,
/// and `Y(s)` is returned.
pub(crate) fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // Polynomial coefficients for X(s) = ax*s^3 + bx*s^2 + cx*s.
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;

    let sample_x = |s: f32| ((ax * s + bx) * s + cx) * s;
    let sample_y = |s: f32| ((ay * s + by) * s + cy) * s;
    let sample_dx = |s: f32| (3.0 * ax * s + 2.0 * bx) * s + cx;

    const EPSILON: f32 = 1e-6;

    // Newton's method, seeded with the input as the initial parametric guess.
    let mut s = t;
    for _ in 0..8 {
        let x = sample_x(s) - t;
        if x.abs() < EPSILON {
            return sample_y(s);
        }
        let dx = sample_dx(s);
        if dx.abs() < EPSILON {
            break;
        }
        s -= x / dx;
    }

    // Bisection fallback for cases where Newton fails to converge.
    let mut lo = 0.0;
    let mut hi = 1.0;
    let mut s = t.clamp(lo, hi);
    for _ in 0..32 {
        let x = sample_x(s);
        if (x - t).abs() < EPSILON {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) * 0.5;
    }
    sample_y(s)
}

/// Evaluates a CSS `steps(n, position)` easing at `progress`.
///
/// The per-variant math follows the CSS Easing Functions spec:
/// - `jump-end`: `floor(p * n) / n`
/// - `jump-start`: `(floor(p * n) + 1) / n`
/// - `jump-none`: `floor(p * n) / (n - 1)` (both endpoints held)
/// - `jump-both`: `(floor(p * n) + 1) / (n + 1)`
pub(crate) fn steps(count: u32, position: StepPosition, progress: f32) -> f32 {
    let p = progress.clamp(0.0, 1.0);
    let n = count as f32;
    let step = (p * n).floor();

    let value = match position {
        StepPosition::JumpEnd => step / n,
        StepPosition::JumpStart => (step + 1.0) / n,
        StepPosition::JumpNone => {
            if count <= 1 {
                0.0
            } else {
                step / (n - 1.0)
            }
        }
        StepPosition::JumpBoth => (step + 1.0) / (n + 1.0),
    };

    value.clamp(0.0, 1.0)
}

/// Evaluates a single SMIL `keySplines` cubic-bezier segment at `progress`.
///
/// `spline` is the `[x1, y1, x2, y2]` control tuple for the active keyframe
/// segment and `progress` is the normalized position within that segment.
pub(crate) fn key_spline(spline: [f32; 4], progress: f32) -> f32 {
    cubic_bezier(spline[0], spline[1], spline[2], spline[3], progress)
}

/// Applies a CSS `TimingFunction` to a linear `progress` in `[0, 1]`.
pub(crate) fn apply_timing_function(tf: &TimingFunction, progress: f32) -> f32 {
    match *tf {
        TimingFunction::Linear => progress.clamp(0.0, 1.0),
        TimingFunction::CubicBezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, progress),
        TimingFunction::Steps(count, position) => steps(count, position, progress),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EASE: (f32, f32, f32, f32) = (0.25, 0.1, 0.25, 1.0);
    const EASE_IN: (f32, f32, f32, f32) = (0.42, 0.0, 1.0, 1.0);
    const EASE_OUT: (f32, f32, f32, f32) = (0.0, 0.0, 0.58, 1.0);
    const EASE_IN_OUT: (f32, f32, f32, f32) = (0.42, 0.0, 0.58, 1.0);

    fn keyword_bezier(keyword: &str) -> Option<(f32, f32, f32, f32)> {
        match keyword {
            "ease" => Some(EASE),
            "ease-in" => Some(EASE_IN),
            "ease-out" => Some(EASE_OUT),
            "ease-in-out" => Some(EASE_IN_OUT),
            _ => None,
        }
    }

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "expected {b}, got {a}");
    }

    #[test]
    fn cubic_bezier_endpoints() {
        approx(cubic_bezier(0.42, 0.0, 0.58, 1.0, 0.0), 0.0);
        approx(cubic_bezier(0.42, 0.0, 0.58, 1.0, 1.0), 1.0);
        // Out-of-range input is clamped.
        approx(cubic_bezier(0.42, 0.0, 0.58, 1.0, -0.5), 0.0);
        approx(cubic_bezier(0.42, 0.0, 0.58, 1.0, 1.5), 1.0);
    }

    #[test]
    fn cubic_bezier_linear_is_identity() {
        // A bezier with collinear controls behaves like the identity.
        for &t in &[0.1_f32, 0.25, 0.5, 0.75, 0.9] {
            approx(
                cubic_bezier(1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, t),
                t,
            );
        }
    }

    #[test]
    fn cubic_bezier_ease_midpoint() {
        // The `ease` curve accelerates early, so at t=0.5 y > 0.5.
        let y = cubic_bezier(EASE.0, EASE.1, EASE.2, EASE.3, 0.5);
        assert!(y > 0.5, "ease at 0.5 should exceed 0.5, got {y}");
    }

    #[test]
    fn cubic_bezier_symmetric_ease_in_out() {
        // ease-in-out is symmetric about (0.5, 0.5).
        let a = cubic_bezier(0.42, 0.0, 0.58, 1.0, 0.25);
        let b = cubic_bezier(0.42, 0.0, 0.58, 1.0, 0.75);
        approx(a + b, 1.0);
    }

    #[test]
    fn steps_jump_end_quantization() {
        approx(steps(4, StepPosition::JumpEnd, 0.0), 0.0);
        approx(steps(4, StepPosition::JumpEnd, 0.24), 0.0);
        approx(steps(4, StepPosition::JumpEnd, 0.25), 0.25);
        approx(steps(4, StepPosition::JumpEnd, 0.5), 0.5);
        approx(steps(4, StepPosition::JumpEnd, 0.75), 0.75);
        approx(steps(4, StepPosition::JumpEnd, 0.99), 0.75);
        approx(steps(4, StepPosition::JumpEnd, 1.0), 1.0);
    }

    #[test]
    fn steps_jump_start_quantization() {
        approx(steps(4, StepPosition::JumpStart, 0.0), 0.25);
        approx(steps(4, StepPosition::JumpStart, 0.24), 0.25);
        approx(steps(4, StepPosition::JumpStart, 0.25), 0.5);
        approx(steps(4, StepPosition::JumpStart, 0.75), 1.0);
        // (floor(1*4)+1)/4 = 1.25 clamped to 1.0.
        approx(steps(4, StepPosition::JumpStart, 1.0), 1.0);
    }

    #[test]
    fn steps_jump_none_quantization() {
        // n steps, divisor n-1: endpoints 0 and 1 are both held.
        approx(steps(4, StepPosition::JumpNone, 0.0), 0.0);
        approx(steps(4, StepPosition::JumpNone, 0.25), 1.0 / 3.0);
        approx(steps(4, StepPosition::JumpNone, 0.5), 2.0 / 3.0);
        approx(steps(4, StepPosition::JumpNone, 0.75), 1.0);
        approx(steps(4, StepPosition::JumpNone, 1.0), 1.0);
        // Degenerate single step is constant 0.
        approx(steps(1, StepPosition::JumpNone, 0.5), 0.0);
    }

    #[test]
    fn steps_jump_both_quantization() {
        // divisor n+1.
        approx(steps(4, StepPosition::JumpBoth, 0.0), 0.2);
        approx(steps(4, StepPosition::JumpBoth, 0.25), 0.4);
        approx(steps(4, StepPosition::JumpBoth, 0.5), 0.6);
        approx(steps(4, StepPosition::JumpBoth, 0.75), 0.8);
        approx(steps(4, StepPosition::JumpBoth, 1.0), 1.0);
    }

    #[test]
    fn key_spline_matches_cubic_bezier() {
        let spline = [0.42, 0.0, 0.58, 1.0];
        approx(
            key_spline(spline, 0.3),
            cubic_bezier(0.42, 0.0, 0.58, 1.0, 0.3),
        );
    }

    #[test]
    fn keyword_bezier_lookup() {
        assert!(keyword_bezier("ease").is_some());
        assert_eq!(keyword_bezier("ease-in"), Some(EASE_IN));
        assert_eq!(keyword_bezier("ease-out"), Some(EASE_OUT));
        assert_eq!(keyword_bezier("ease-in-out"), Some(EASE_IN_OUT));
        assert_eq!(keyword_bezier("linear"), None);
    }

    #[test]
    fn apply_timing_function_dispatch() {
        approx(apply_timing_function(&TimingFunction::Linear, 0.42), 0.42);
        approx(
            apply_timing_function(&TimingFunction::Steps(4, StepPosition::JumpEnd), 0.6),
            0.5,
        );
        approx(
            apply_timing_function(&TimingFunction::CubicBezier(0.42, 0.0, 0.58, 1.0), 0.0),
            0.0,
        );
    }
}
