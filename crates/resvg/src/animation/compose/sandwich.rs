// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use usvg::{Animation, Dur, Interval, NodeAnimation, SmilFill, SmilTiming, Timing};

use super::super::timing::{css_progress, smil_progress};
use super::SampledOverrides;
use super::apply::{ImageState, fold};

/// One animation that contributes to the sandwich at the query time.
pub(super) struct Contribution<'a> {
    pub(super) animation: &'a Animation,
    pub(super) progress: f32,
    pub(super) iteration: u32,
    css: bool,
    begin: f32,
    pub(super) order: usize,
}

/// Samples every animation on `node_anim` at time `t` and folds them into the
/// per-attribute sandwich.
pub(crate) fn sample_overrides(node_anim: &NodeAnimation, t: f32) -> SampledOverrides {
    let mut overrides = SampledOverrides::default();
    if node_anim.base_hidden() {
        overrides.hidden = Some(true);
    }
    let mut image = ImageState::new(node_anim);

    let mut contribution_iter = node_anim
        .animations()
        .iter()
        .enumerate()
        .filter(|(_, animation)| !animation.suppressed_by_important())
        .filter_map(|(order, animation)| build_contribution(animation, order, t));

    let Some(first) = contribution_iter.next() else {
        overrides.image_geometry = image.finish();
        return overrides;
    };

    let Some(second) = contribution_iter.next() else {
        fold(&mut overrides, &mut image, &first);
        overrides.image_geometry = image.finish();
        return overrides;
    };

    let mut contributions = vec![first, second];
    contributions.extend(contribution_iter);

    // SMIL sorts by interval begin (later wins) then document order; CSS sorts
    // after all SMIL contributions, in document order.
    contributions.sort_by(|a, b| {
        a.css
            .cmp(&b.css)
            .then(a.begin.total_cmp(&b.begin))
            .then(a.order.cmp(&b.order))
    });

    for contribution in &contributions {
        fold(&mut overrides, &mut image, contribution);
    }
    overrides.image_geometry = image.finish();
    overrides
}

/// Resolves an animation's progress and priority key at time `t`, or `None` when
/// it contributes nothing.
fn build_contribution(animation: &Animation, order: usize, t: f32) -> Option<Contribution<'_>> {
    match animation.timing() {
        Timing::Smil(smil) => {
            let progress = smil_progress(smil, t)?;
            let (begin, iteration) = smil_interval(smil, t)?;
            Some(Contribution {
                animation,
                progress,
                iteration,
                css: false,
                begin,
                order,
            })
        }
        Timing::Css(css) => {
            let progress = css_progress(css, t)?;
            Some(Contribution {
                animation,
                progress,
                iteration: 0,
                css: true,
                begin: 0.0,
                order,
            })
        }
    }
}

/// Locates the contributing SMIL interval and returns its begin and the 0-based
/// iteration index at `t`, mirroring [`smil_progress`]'s interval selection.
fn smil_interval(timing: &SmilTiming, t: f32) -> Option<(f32, u32)> {
    let mut most_recent: Option<&Interval> = None;
    for interval in timing.intervals() {
        if interval.begin() <= t {
            most_recent = Some(interval);
        }
        let active = match interval.end() {
            Some(end) => interval.begin() <= t && t < end,
            None => interval.begin() <= t,
        };
        if active {
            return Some((
                interval.begin(),
                active_iteration(interval.begin(), t, timing.dur()),
            ));
        }
    }
    let interval = most_recent?;
    match timing.fill() {
        SmilFill::Freeze => Some((interval.begin(), frozen_iteration(interval, timing.dur()))),
        SmilFill::Remove => None,
    }
}

/// The 0-based iteration index for an active interval.
fn active_iteration(begin: f32, t: f32, dur: &Dur) -> u32 {
    match *dur {
        Dur::Seconds(seconds) if seconds > 0.0 => ((t - begin) / seconds).floor().max(0.0) as u32,
        _ => 0,
    }
}

/// The 0-based iteration index held after a frozen interval ends.
///
/// A whole-number boundary freezes at the end of the last completed iteration,
/// matching [`smil_progress`]'s freeze-at-`1.0` rule.
fn frozen_iteration(interval: &Interval, dur: &Dur) -> u32 {
    let Some(end) = interval.end() else {
        return 0;
    };
    match *dur {
        Dur::Seconds(seconds) if seconds > 0.0 => {
            let raw = (end - interval.begin()) / seconds;
            let floored = raw.floor();
            if raw - floored <= f32::EPSILON && raw >= 1.0 {
                (floored - 1.0).max(0.0) as u32
            } else {
                floored.max(0.0) as u32
            }
        }
        _ => 0,
    }
}
