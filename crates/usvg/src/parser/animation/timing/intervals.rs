// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::clock::parse_clock_value;
use super::syncbase::resolve_timing_list;
use crate::parser::svgtree::{AId, NodeId, SvgNode};
use crate::tree::animation::{Direction, Interval, TimedInterval, Timing};

#[derive(Clone, Copy, Debug)]
pub(super) enum Begin {
    Offset(f32),
    Indefinite,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SimpleDuration {
    Seconds(f32),
    Indefinite,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum IterationLimit {
    Count(f32),
    Indefinite,
}

#[derive(Clone, Copy, Debug)]
enum FillBehavior {
    Freeze,
    Remove,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Admission {
    Always,
    Never,
    WhenInactive,
}

/// Parses SMIL timing and bakes its resolved behavior into a canonical timeline.
pub(crate) fn parse_smil_timing<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
) -> Timing {
    if node.has_attribute(AId::Min) {
        log::warn!("Unsupported SMIL timing attribute: '{}'.", "min");
    }
    if node.has_attribute(AId::Max) {
        log::warn!("Unsupported SMIL timing attribute: '{}'.", "max");
    }

    let mut visiting = vec![node];
    let begins = resolve_timing_list(node, AId::Begin, true, all_animations, &mut visiting);
    let ends = resolve_timing_list(node, AId::End, false, all_animations, &mut visiting);
    let parsed_duration = parse_dur(node);
    let limit = parse_repeat_count(node);
    let repeat_duration = parse_repeat_dur(node);
    let fill = parse_fill(node);
    let admission = parse_restart(node);
    let active = active_duration(parsed_duration, limit, repeat_duration);
    let intervals = build_intervals(&begins, &ends, active, admission);
    let iteration_dur = simple_duration(parsed_duration);
    let one_loop_end = iteration_dur.and_then(|duration| {
        intervals
            .iter()
            .map(Interval::begin)
            .reduce(f32::min)
            .map(|begin| begin + duration)
    });
    let intervals = intervals
        .into_iter()
        .map(|interval| {
            let held = match fill {
                FillBehavior::Freeze => Some(frozen_progress(&interval, iteration_dur)),
                FillBehavior::Remove => None,
            };
            TimedInterval::new(interval, held)
        })
        .collect();

    Timing::new(
        intervals,
        iteration_dur,
        Direction::Normal,
        None,
        one_loop_end,
    )
}

pub(super) fn parse_dur<'a, 'input>(node: SvgNode<'a, 'input>) -> SimpleDuration {
    let Some(value) = node.attribute::<&str>(AId::Dur) else {
        return SimpleDuration::Indefinite;
    };

    let value = value.trim();
    if value == "indefinite" || value == "media" {
        return SimpleDuration::Indefinite;
    }

    match parse_clock_value(value) {
        Some(seconds) if seconds >= 0.0 => SimpleDuration::Seconds(seconds),
        _ => SimpleDuration::Indefinite,
    }
}

fn parse_restart<'a, 'input>(node: SvgNode<'a, 'input>) -> Admission {
    match node.attribute::<&str>(AId::Restart) {
        Some("never") => Admission::Never,
        Some("whenNotActive") => Admission::WhenInactive,
        _ => Admission::Always,
    }
}

fn parse_fill<'a, 'input>(node: SvgNode<'a, 'input>) -> FillBehavior {
    match node.attribute::<&str>(AId::Fill) {
        Some("freeze") => FillBehavior::Freeze,
        _ => FillBehavior::Remove,
    }
}

pub(super) fn parse_repeat_count<'a, 'input>(node: SvgNode<'a, 'input>) -> Option<IterationLimit> {
    let value = node.attribute::<&str>(AId::RepeatCount)?;
    let value = value.trim();
    if value == "indefinite" {
        return Some(IterationLimit::Indefinite);
    }

    match value.parse::<f32>() {
        Ok(count) if count.is_finite() && count > 0.0 => Some(IterationLimit::Count(count)),
        _ => None,
    }
}

pub(super) fn parse_repeat_dur<'a, 'input>(node: SvgNode<'a, 'input>) -> Option<f32> {
    let value = node.attribute::<&str>(AId::RepeatDur)?;
    let value = value.trim();
    if value == "indefinite" {
        return None;
    }

    parse_clock_value(value).filter(|seconds| *seconds > 0.0)
}

pub(super) fn active_duration(
    duration: SimpleDuration,
    limit: Option<IterationLimit>,
    repeat_duration: Option<f32>,
) -> Option<f32> {
    let simple = match duration {
        SimpleDuration::Seconds(seconds) => Some(seconds),
        SimpleDuration::Indefinite => None,
    };

    if limit.is_none() && repeat_duration.is_none() {
        return simple;
    }

    let by_count = match limit {
        Some(IterationLimit::Count(count)) => simple.map(|seconds| count * seconds),
        Some(IterationLimit::Indefinite) | None => None,
    };

    match (by_count, repeat_duration) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn simple_duration(duration: SimpleDuration) -> Option<f32> {
    match duration {
        SimpleDuration::Seconds(seconds) if seconds > 0.0 => Some(seconds),
        SimpleDuration::Seconds(_) | SimpleDuration::Indefinite => None,
    }
}

fn frozen_progress(interval: &Interval, iteration_dur: Option<f32>) -> f32 {
    let Some(end) = interval.end() else {
        return 0.0;
    };
    let Some(duration) = iteration_dur else {
        return 1.0;
    };

    let raw = (end - interval.begin()) / duration;
    let fraction = raw - raw.floor();
    if fraction <= f32::EPSILON && raw >= 1.0 {
        1.0
    } else {
        fraction
    }
}

fn build_intervals(
    begins: &[Begin],
    ends: &[Begin],
    active: Option<f32>,
    admission: Admission,
) -> Vec<Interval> {
    let mut begin_instants: Vec<f32> = begins.iter().filter_map(begin_instant).collect();
    begin_instants.sort_by(cmp_f32);
    begin_instants.dedup();

    let mut end_instants: Vec<f32> = ends.iter().filter_map(begin_instant).collect();
    end_instants.sort_by(cmp_f32);
    let has_end_list = !end_instants.is_empty();

    let mut intervals = Vec::new();
    let mut last_end = f32::NEG_INFINITY;

    for (index, &begin) in begin_instants.iter().enumerate() {
        let active_end = active.map(|duration| begin + duration);
        let mut end = if has_end_list {
            match end_instants.iter().copied().find(|&end| end >= begin) {
                Some(end) => Some(active_end.map_or(end, |active_end| active_end.min(end))),
                None => match active_end {
                    Some(active_end) => Some(active_end),
                    None => continue,
                },
            }
        } else {
            active_end
        };

        if matches!(admission, Admission::Always) {
            if let Some(&next_begin) = begin_instants.get(index + 1) {
                end = Some(end.map_or(next_begin, |current| current.min(next_begin)));
            }
        }

        let admit = match admission {
            Admission::Never => intervals.is_empty(),
            Admission::WhenInactive => begin >= last_end,
            Admission::Always => true,
        };
        if !admit {
            continue;
        }
        if end.is_some_and(|end| end < begin) {
            continue;
        }

        intervals.push(Interval::new(begin, end));
        last_end = end.unwrap_or(f32::INFINITY);
    }

    intervals
}

pub(super) fn begin_instant(begin: &Begin) -> Option<f32> {
    match begin {
        Begin::Offset(seconds) => Some(*seconds),
        Begin::Indefinite => None,
    }
}

fn cmp_f32(a: &f32, b: &f32) -> std::cmp::Ordering {
    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
}
