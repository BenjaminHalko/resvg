// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::clock::parse_clock_value;
use super::syncbase::resolve_timing_list;
use crate::parser::svgtree::{AId, NodeId, SvgNode};
use crate::tree::animation::{Begin, Dur, Interval, RepeatCount, Restart, SmilFill, SmilTiming};

/// Parses the SMIL timing attributes of an animation `node` into a [`SmilTiming`].
///
/// `all_animations` lists every animation element in the document so syncbase
/// references (`x.begin`, `x.end`) can be resolved. Event-based, cyclic and
/// otherwise unresolvable values are dropped with a warning.
pub(crate) fn parse_smil_timing<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
) -> SmilTiming {
    if node.has_attribute(AId::Min) {
        log::warn!("Unsupported SMIL timing attribute: '{}'.", "min");
    }
    if node.has_attribute(AId::Max) {
        log::warn!("Unsupported SMIL timing attribute: '{}'.", "max");
    }

    let mut visiting = vec![node];
    let begins = resolve_timing_list(node, AId::Begin, true, all_animations, &mut visiting);
    let ends = resolve_timing_list(node, AId::End, false, all_animations, &mut visiting);

    let dur = parse_dur(node);
    let repeat_count = parse_repeat_count(node);
    let repeat_dur = parse_repeat_dur(node);
    let fill = parse_fill(node);
    let restart = parse_restart(node);

    let active = active_duration(dur, repeat_count, repeat_dur);
    let intervals = build_intervals(&begins, &ends, active, restart);

    SmilTiming::new(
        begins,
        dur,
        ends,
        repeat_count,
        repeat_dur,
        fill,
        restart,
        intervals,
    )
}

/// Parses the `dur` attribute. Absent or invalid durations are `indefinite`.
pub(super) fn parse_dur<'a, 'input>(node: SvgNode<'a, 'input>) -> Dur {
    let Some(value) = node.attribute::<&str>(AId::Dur) else {
        return Dur::Indefinite;
    };

    let value = value.trim();
    if value == "indefinite" || value == "media" {
        return Dur::Indefinite;
    }

    match parse_clock_value(value) {
        Some(seconds) if seconds >= 0.0 => Dur::Seconds(seconds),
        _ => Dur::Indefinite,
    }
}

/// Parses the `restart` attribute, defaulting to `always`.
fn parse_restart<'a, 'input>(node: SvgNode<'a, 'input>) -> Restart {
    match node.attribute::<&str>(AId::Restart) {
        Some("never") => Restart::Never,
        Some("whenNotActive") => Restart::WhenNotActive,
        _ => Restart::Always,
    }
}

/// Parses the SMIL `fill` attribute, defaulting to `remove`.
fn parse_fill<'a, 'input>(node: SvgNode<'a, 'input>) -> SmilFill {
    match node.attribute::<&str>(AId::Fill) {
        Some("freeze") => SmilFill::Freeze,
        _ => SmilFill::Remove,
    }
}

/// Parses the `repeatCount` attribute.
pub(super) fn parse_repeat_count<'a, 'input>(node: SvgNode<'a, 'input>) -> Option<RepeatCount> {
    let value = node.attribute::<&str>(AId::RepeatCount)?;
    let value = value.trim();
    if value == "indefinite" {
        return Some(RepeatCount::Indefinite);
    }

    match value.parse::<f32>() {
        Ok(count) if count.is_finite() && count > 0.0 => Some(RepeatCount::Count(count)),
        _ => None,
    }
}

/// Parses the `repeatDur` attribute. `indefinite` yields `None` (unbounded).
pub(super) fn parse_repeat_dur<'a, 'input>(node: SvgNode<'a, 'input>) -> Option<f32> {
    let value = node.attribute::<&str>(AId::RepeatDur)?;
    let value = value.trim();
    if value == "indefinite" {
        return None;
    }

    parse_clock_value(value).filter(|seconds| *seconds > 0.0)
}

/// Computes the active duration in seconds, or `None` when it is indefinite.
///
/// Follows the SMIL rule `min(repeatCount * dur, repeatDur)`, treating an
/// indefinite operand as unbounded.
pub(super) fn active_duration(
    dur: Dur,
    repeat_count: Option<RepeatCount>,
    repeat_dur: Option<f32>,
) -> Option<f32> {
    let simple = match dur {
        Dur::Seconds(seconds) => Some(seconds),
        Dur::Indefinite => None,
    };

    if repeat_count.is_none() && repeat_dur.is_none() {
        return simple;
    }

    let by_count = match repeat_count {
        Some(RepeatCount::Count(count)) => simple.map(|seconds| count * seconds),
        Some(RepeatCount::Indefinite) => None,
        None => None,
    };

    match (by_count, repeat_dur) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Builds the resolved intervals from the resolved begin/end lists.
///
/// Each interval end is computed before the restart admission decision:
/// `restart=never` keeps only the first interval, `restart=whenNotActive` skips
/// a begin that falls inside the previous interval, and `restart=always` accepts
/// every begin while capping each interval at the next begin.
fn build_intervals(
    begins: &[Begin],
    ends: &[Begin],
    active: Option<f32>,
    restart: Restart,
) -> Vec<Interval> {
    let mut begin_instants: Vec<f32> = begins.iter().filter_map(begin_instant).collect();
    begin_instants.sort_by(cmp_f32);
    begin_instants.dedup();

    let mut end_instants: Vec<f32> = ends.iter().filter_map(begin_instant).collect();
    end_instants.sort_by(cmp_f32);
    let has_end_list = !end_instants.is_empty();

    let mut intervals = Vec::new();
    let mut last_end = f32::NEG_INFINITY;

    for (i, &begin) in begin_instants.iter().enumerate() {
        let active_end = active.map(|duration| begin + duration);

        let mut end = if has_end_list {
            match end_instants.iter().copied().find(|&e| e >= begin) {
                Some(e) => Some(active_end.map_or(e, |a| a.min(e))),
                None => match active_end {
                    Some(a) => Some(a),
                    None => continue,
                },
            }
        } else {
            active_end
        };

        if matches!(restart, Restart::Always) {
            if let Some(&next_begin) = begin_instants.get(i + 1) {
                end = Some(end.map_or(next_begin, |cur| cur.min(next_begin)));
            }
        }

        let admit = match restart {
            Restart::Never => intervals.is_empty(),
            Restart::WhenNotActive => begin >= last_end,
            Restart::Always => true,
        };
        if !admit {
            continue;
        }

        if let Some(e) = end {
            if e < begin {
                continue;
            }
        }

        intervals.push(Interval::new(begin, end));
        last_end = end.unwrap_or(f32::INFINITY);
    }

    intervals
}

/// Returns the offset of a [`Begin`], or `None` for `indefinite`.
pub(super) fn begin_instant(begin: &Begin) -> Option<f32> {
    match begin {
        Begin::Offset(seconds) => Some(*seconds),
        Begin::Indefinite => None,
    }
}

/// Total ordering for the finite instants produced during resolution.
fn cmp_f32(a: &f32, b: &f32) -> std::cmp::Ordering {
    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
}
