// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use svgtypes::Color;
use usvg::{Easing, Keyframe, NonZeroRect, NormalizedF32, StrokeMiterlimit, TimingFunction};

use super::locate::{lerp, locate_track, paced_of};

/// Samples a plain `f32` track (stroke width, geometry, image geometry).
pub(super) fn sample_scalar(
    keyframes: &[Keyframe<f32>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<f32> {
    let paced = paced_of(keyframes, easing, |a, b| (a - b).abs());
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
    Some(lerp(*keyframes[lo].value(), *keyframes[hi].value(), t))
}

/// Samples a normalized track (`opacity`, `stop-opacity`, `stop` offset).
pub(super) fn sample_opacity(
    keyframes: &[Keyframe<NormalizedF32>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<f32> {
    let paced = paced_of(keyframes, easing, |a, b| (a.get() - b.get()).abs());
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
    let value = lerp(keyframes[lo].value().get(), keyframes[hi].value().get(), t);
    Some(value.clamp(0.0, 1.0))
}

/// Samples a `stroke-miterlimit` track.
pub(super) fn sample_miterlimit(
    keyframes: &[Keyframe<StrokeMiterlimit>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<f32> {
    let paced = paced_of(keyframes, easing, |a, b| (a.get() - b.get()).abs());
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
    Some(lerp(
        keyframes[lo].value().get(),
        keyframes[hi].value().get(),
        t,
    ))
}

/// Samples a color track by lerping each sRGB channel.
pub(super) fn sample_color(
    keyframes: &[Keyframe<Color>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Color> {
    let paced = paced_of(keyframes, easing, color_distance);
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
    Some(lerp_color(keyframes[lo].value(), keyframes[hi].value(), t))
}

/// Lerps two colors channel-wise in sRGB space.
fn lerp_color(a: &Color, b: &Color, t: f32) -> Color {
    Color::new_rgba(
        lerp_channel(a.red, b.red, t),
        lerp_channel(a.green, b.green, t),
        lerp_channel(a.blue, b.blue, t),
        lerp_channel(a.alpha, b.alpha, t),
    )
}

/// Lerps a single 8-bit color channel, rounding to the nearest value.
fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    lerp(f32::from(a), f32::from(b), t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// The Euclidean distance between two colors over RGBA channels.
fn color_distance(a: &Color, b: &Color) -> f32 {
    let dr = f32::from(a.red) - f32::from(b.red);
    let dg = f32::from(a.green) - f32::from(b.green);
    let db = f32::from(a.blue) - f32::from(b.blue);
    let da = f32::from(a.alpha) - f32::from(b.alpha);
    (dr * dr + dg * dg + db * db + da * da).sqrt()
}

/// Samples a `stroke-dasharray` track element-wise.
pub(super) fn sample_dasharray(
    keyframes: &[Keyframe<Vec<f32>>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<Vec<f32>> {
    let paced = paced_of(keyframes, easing, |a, b| {
        let len = a.len().min(b.len());
        (0..len).map(|i| (a[i] - b[i]).abs()).sum()
    });
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
    let a = keyframes[lo].value();
    let b = keyframes[hi].value();
    let len = a.len().min(b.len());
    Some((0..len).map(|i| lerp(a[i], b[i], t)).collect())
}

/// Samples a discrete-only track (enums, `display`) by holding the low keyframe.
pub(super) fn sample_discrete<T: Copy>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<T> {
    let (lo, _, _) = locate_track(keyframes, easing, timing_function, progress, None)?;
    Some(*keyframes[lo].value())
}

/// Samples a visibility track with its source value retained at a keyframe boundary.
pub(super) fn sample_discrete_before_boundary<T: Copy>(
    keyframes: &[Keyframe<T>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<T> {
    let progress = if progress < 1.0 {
        progress.next_down()
    } else {
        progress
    };
    sample_discrete(keyframes, easing, timing_function, progress)
}

/// Samples a `viewBox` track by lerping each rect component.
pub(super) fn sample_viewbox(
    keyframes: &[Keyframe<NonZeroRect>],
    easing: &Easing,
    timing_function: Option<&TimingFunction>,
    progress: f32,
) -> Option<NonZeroRect> {
    let paced = paced_of(keyframes, easing, |a, b| {
        let dx = a.x() - b.x();
        let dy = a.y() - b.y();
        let dw = a.width() - b.width();
        let dh = a.height() - b.height();
        (dx * dx + dy * dy + dw * dw + dh * dh).sqrt()
    });
    let (lo, hi, t) = locate_track(keyframes, easing, timing_function, progress, paced)?;
    let a = keyframes[lo].value();
    let b = keyframes[hi].value();
    NonZeroRect::from_xywh(
        lerp(a.x(), b.x(), t),
        lerp(a.y(), b.y(), t),
        lerp(a.width(), b.width(), t),
        lerp(a.height(), b.height(), t),
    )
}
