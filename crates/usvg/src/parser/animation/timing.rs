// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::NormalizedF32;
use crate::parser::svgtree::{AId, EId, NodeId, SvgNode};
use crate::tree::animation::{
    Begin, CalcMode, Dur, Easing, Interval, RepeatCount, Restart, SmilFill, SmilTiming,
};

/// A partially-parsed `begin`/`end` value, before syncbase resolution.
#[derive(Clone, Debug)]
pub(crate) enum RawBegin {
    /// A resolved time offset in seconds.
    Offset(f32),
    /// The `indefinite` value.
    Indefinite,
    /// A syncbase reference (`id.begin`/`id.end`) with an offset.
    SyncBase {
        id: String,
        edge: SyncEdge,
        offset: f32,
    },
}

/// The referenced timing edge of a syncbase value.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SyncEdge {
    /// The `begin` edge.
    Begin,
    /// The `end` edge.
    End,
}

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

/// Resolves a `begin` or `end` attribute into a list of [`Begin`] values.
///
/// An absent attribute yields `[Begin::Offset(0.0)]` when `default_zero` is set
/// (the `begin` default) or an empty list otherwise (the `end` default). A
/// present-but-fully-invalid list yields an empty list.
fn resolve_timing_list<'a, 'input>(
    node: SvgNode<'a, 'input>,
    aid: AId,
    default_zero: bool,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Vec<Begin> {
    let Some(value) = node.attribute::<&str>(aid) else {
        return if default_zero {
            vec![Begin::Offset(0.0)]
        } else {
            Vec::new()
        };
    };

    let mut out = Vec::new();
    for entry in value.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let resolved =
            parse_begin_entry(entry).and_then(|raw| resolve_raw(&raw, all_animations, visiting));
        match resolved {
            Some(begin) => out.push(begin),
            None => log::warn!("Unsupported animation begin/end value: '{}'.", entry),
        }
    }
    out
}

/// Resolves a [`RawBegin`] into a concrete [`Begin`], or `None` when a syncbase
/// value cannot be statically resolved.
fn resolve_raw<'a, 'input>(
    raw: &RawBegin,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<Begin> {
    match raw {
        RawBegin::Offset(seconds) => Some(Begin::Offset(*seconds)),
        RawBegin::Indefinite => Some(Begin::Indefinite),
        RawBegin::SyncBase { id, edge, offset } => {
            resolve_syncbase(id, *edge, *offset, all_animations, visiting).map(Begin::Offset)
        }
    }
}

/// Resolves `id.begin`/`id.end` plus an offset to an absolute instant.
///
/// `x.begin` resolves when `x` has a single static begin. `x.end` resolves only
/// when `x` is statically determinate: a single begin, no explicit end list and
/// a finite active duration. Cyclic references return `None`.
fn resolve_syncbase<'a, 'input>(
    id: &str,
    edge: SyncEdge,
    offset: f32,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<f32> {
    let target = find_animation_node(id, all_animations)?;
    if visiting.contains(&target) {
        return None;
    }

    visiting.push(target);
    let base = match edge {
        SyncEdge::Begin => resolve_single_begin(target, all_animations, visiting),
        SyncEdge::End => resolve_end_instant(target, all_animations, visiting),
    };
    visiting.pop();

    base.map(|seconds| seconds + offset)
}

/// Finds the animation element with the given `id`.
fn find_animation_node<'a, 'input>(
    id: &str,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
) -> Option<SvgNode<'a, 'input>> {
    all_animations
        .iter()
        .find(|(_, node)| node.element_id() == id)
        .map(|(_, node)| *node)
}

/// Resolves a node's begins, returning the instant only when exactly one static
/// begin exists.
fn resolve_single_begin<'a, 'input>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<f32> {
    let mut instants = Vec::new();
    for raw in raw_begin_list(node) {
        if let Some(Begin::Offset(seconds)) = resolve_raw(&raw, all_animations, visiting) {
            instants.push(seconds);
        }
    }
    (instants.len() == 1).then(|| instants[0])
}

/// Resolves a node's end instant when it is statically determinate.
fn resolve_end_instant<'a, 'input>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<f32> {
    if has_end_entries(node) {
        return None;
    }

    let begin = resolve_single_begin(node, all_animations, visiting)?;
    let active = active_duration(
        parse_dur(node),
        parse_repeat_count(node),
        parse_repeat_dur(node),
    )?;
    Some(begin + active)
}

/// Parses a node's `begin` list into [`RawBegin`] values without warnings.
///
/// An absent `begin` defaults to a single zero offset.
fn raw_begin_list<'a, 'input>(node: SvgNode<'a, 'input>) -> Vec<RawBegin> {
    let Some(value) = node.attribute::<&str>(AId::Begin) else {
        return vec![RawBegin::Offset(0.0)];
    };

    let mut out = Vec::new();
    for entry in value.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some(raw) = parse_begin_entry(entry) {
            out.push(raw);
        }
    }
    out
}

/// Returns whether a node has a non-empty `end` list.
fn has_end_entries<'a, 'input>(node: SvgNode<'a, 'input>) -> bool {
    node.attribute::<&str>(AId::End)
        .map(|value| value.split(';').any(|entry| !entry.trim().is_empty()))
        .unwrap_or(false)
}

/// Parses a single `begin`/`end` list entry into a [`RawBegin`].
///
/// Offsets and `indefinite` resolve immediately; syncbase values are kept for
/// topological resolution. Event-based and otherwise unsupported values return
/// `None`.
fn parse_begin_entry(entry: &str) -> Option<RawBegin> {
    let entry = entry.trim();
    if entry.is_empty() {
        return None;
    }
    if entry == "indefinite" {
        return Some(RawBegin::Indefinite);
    }

    let first = entry.as_bytes()[0];
    if first == b'+' || first == b'-' || first == b'.' || first.is_ascii_digit() {
        return parse_offset(entry).map(RawBegin::Offset);
    }

    parse_syncbase(entry)
}

/// Parses a syncbase value of the form `id.begin`/`id.end` with an optional
/// offset.
fn parse_syncbase(entry: &str) -> Option<RawBegin> {
    for (edge, marker) in [(SyncEdge::Begin, ".begin"), (SyncEdge::End, ".end")] {
        let mut search = 0;
        while let Some(pos) = entry[search..].find(marker) {
            let index = search + pos;
            let id = &entry[..index];
            let rest = &entry[index + marker.len()..];
            if !id.is_empty() {
                if let Some(offset) = parse_sync_offset(rest) {
                    return Some(RawBegin::SyncBase {
                        id: id.to_string(),
                        edge,
                        offset,
                    });
                }
            }
            search = index + 1;
        }
    }
    None
}

/// Parses the trailing offset of a syncbase value, or `0.0` when absent.
fn parse_sync_offset(rest: &str) -> Option<f32> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(0.0);
    }
    if !rest.starts_with('+') && !rest.starts_with('-') {
        return None;
    }
    parse_offset(rest)
}

/// Parses an optionally-signed clock value offset.
fn parse_offset(value: &str) -> Option<f32> {
    let value = value.trim();
    let (sign, rest) = if let Some(rest) = value.strip_prefix('+') {
        (1.0, rest)
    } else if let Some(rest) = value.strip_prefix('-') {
        (-1.0, rest)
    } else {
        (1.0, value)
    };
    parse_clock_value(rest.trim()).map(|seconds| sign * seconds)
}

/// Parses a SMIL clock value (e.g. `4`, `3s`, `1.5s`, `02:30`, `1min`, `500ms`)
/// into seconds.
fn parse_clock_value(value: &str) -> Option<f32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value.contains(':') {
        return parse_clock_colon(value);
    }

    if let Some(number) = value.strip_suffix("ms") {
        return parse_number(number).map(|v| v / 1000.0);
    }
    if let Some(number) = value.strip_suffix("min") {
        return parse_number(number).map(|v| v * 60.0);
    }
    if let Some(number) = value.strip_suffix('h') {
        return parse_number(number).map(|v| v * 3600.0);
    }
    if let Some(number) = value.strip_suffix('s') {
        return parse_number(number);
    }

    parse_number(value)
}

/// Parses the `HH:MM:SS(.fff)` and `MM:SS(.fff)` clock forms.
fn parse_clock_colon(value: &str) -> Option<f32> {
    let mut parts = value.split(':');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }

    let (hours, minutes, seconds) = match third {
        Some(third) => (
            parse_number(first)?,
            parse_number(second)?,
            parse_number(third)?,
        ),
        None => (0.0, parse_number(first)?, parse_number(second)?),
    };

    if hours < 0.0 || minutes < 0.0 || seconds < 0.0 {
        return None;
    }

    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Parses a finite `f32`.
fn parse_number(value: &str) -> Option<f32> {
    let number: f32 = value.trim().parse().ok()?;
    number.is_finite().then_some(number)
}

/// Parses the `dur` attribute. Absent or invalid durations are `indefinite`.
fn parse_dur<'a, 'input>(node: SvgNode<'a, 'input>) -> Dur {
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
fn parse_repeat_count<'a, 'input>(node: SvgNode<'a, 'input>) -> Option<RepeatCount> {
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
fn parse_repeat_dur<'a, 'input>(node: SvgNode<'a, 'input>) -> Option<f32> {
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
fn active_duration(
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
fn begin_instant(begin: &Begin) -> Option<f32> {
    match begin {
        Begin::Offset(seconds) => Some(*seconds),
        Begin::Indefinite => None,
    }
}

/// Total ordering for the finite instants produced during resolution.
fn cmp_f32(a: &f32, b: &f32) -> std::cmp::Ordering {
    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
}

/// Parses `calcMode`, `keyTimes` and `keySplines` into an [`Easing`].
///
/// `values_count` is the number of animation values, used to validate the
/// `keyTimes`/`keySplines` counts. Invalid timing is dropped (returns `None`)
/// with a warning. `calcMode=paced` ignores `keyTimes` and `keySplines`.
pub(crate) fn parse_easing<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
    values_count: usize,
) -> Option<Easing> {
    let calc_mode = parse_calc_mode(node);

    if matches!(calc_mode, CalcMode::Paced) {
        return Some(Easing::new(CalcMode::Paced, None, None));
    }

    let key_times = match node.attribute::<&str>(AId::KeyTimes) {
        Some(raw) => match parse_key_times(raw) {
            Some(times) if valid_key_times(&times, calc_mode, values_count) => Some(times),
            _ => {
                log::warn!("Invalid animation timing: '{}'.", raw);
                return None;
            }
        },
        None => None,
    };

    let key_splines = if matches!(calc_mode, CalcMode::Spline) {
        let raw = node.attribute::<&str>(AId::KeySplines).unwrap_or("");
        let expected = match &key_times {
            Some(times) => times.len().saturating_sub(1),
            None => values_count.saturating_sub(1),
        };
        match parse_key_splines(raw) {
            Some(splines) if splines.len() == expected && splines_in_range(&splines) => {
                Some(splines)
            }
            _ => {
                log::warn!("Invalid animation timing: '{}'.", raw);
                return None;
            }
        }
    } else {
        None
    };

    let key_times = key_times.map(|times| {
        times
            .into_iter()
            .map(NormalizedF32::new_clamped)
            .collect::<Vec<_>>()
    });

    Some(Easing::new(calc_mode, key_times, key_splines))
}

/// Parses `calcMode`, defaulting to `paced` for `<animateMotion>` and `linear`
/// otherwise.
fn parse_calc_mode<'a, 'input>(node: SvgNode<'a, 'input>) -> CalcMode {
    match node.attribute::<&str>(AId::CalcMode) {
        Some("discrete") => CalcMode::Discrete,
        Some("linear") => CalcMode::Linear,
        Some("paced") => CalcMode::Paced,
        Some("spline") => CalcMode::Spline,
        _ => {
            if node.tag_name() == Some(EId::AnimateMotion) {
                CalcMode::Paced
            } else {
                CalcMode::Linear
            }
        }
    }
}

/// Parses a `keyTimes` value into raw numbers.
fn parse_key_times(value: &str) -> Option<Vec<f32>> {
    let mut times = Vec::new();
    for part in value.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        times.push(parse_number(part)?);
    }
    (!times.is_empty()).then_some(times)
}

/// Parses a `keySplines` value into cubic Bézier control point quadruples.
fn parse_key_splines(value: &str) -> Option<Vec<[f32; 4]>> {
    let mut splines = Vec::new();
    for group in value.split(';') {
        let group = group.trim();
        if group.is_empty() {
            continue;
        }

        let mut spline = [0.0f32; 4];
        let mut count = 0;
        for token in group.split([',', ' ', '\t', '\n', '\r']) {
            if token.is_empty() {
                continue;
            }
            if count == 4 {
                return None;
            }
            spline[count] = parse_number(token)?;
            count += 1;
        }

        if count != 4 {
            return None;
        }
        splines.push(spline);
    }
    (!splines.is_empty()).then_some(splines)
}

/// Validates `keyTimes` against the effective `calcMode`.
///
/// All modes require finite values in `[0, 1]`, a leading `0`, a monotonically
/// non-decreasing sequence and a count equal to the number of values. `discrete`
/// waives the trailing `1` requirement.
fn valid_key_times(times: &[f32], calc_mode: CalcMode, values_count: usize) -> bool {
    if times.len() != values_count {
        return false;
    }
    if times.first() != Some(&0.0) {
        return false;
    }
    if times.iter().any(|t| !(0.0f32..=1.0).contains(t)) {
        return false;
    }
    if times.windows(2).any(|w| w[0] > w[1]) {
        return false;
    }
    if !matches!(calc_mode, CalcMode::Discrete) && times.last() != Some(&1.0) {
        return false;
    }
    true
}

/// Checks that every `keySplines` control value is within `[0, 1]`.
fn splines_in_range(splines: &[[f32; 4]]) -> bool {
    splines.iter().flatten().all(|v| (0.0f32..=1.0).contains(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::svgtree::Document;
    use std::cell::RefCell;
    use std::sync::Once;

    const NS: &str = "http://www.w3.org/2000/svg";

    thread_local! {
        static WARNINGS: RefCell<Option<Vec<String>>> = RefCell::new(None);
    }

    struct CaptureLogger;

    impl log::Log for CaptureLogger {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            WARNINGS.with(|slot| {
                if let Some(buffer) = slot.borrow_mut().as_mut() {
                    buffer.push(format!("{}", record.args()));
                }
            });
        }

        fn flush(&self) {}
    }

    /// Captures the warnings emitted on the current thread while `f` runs.
    fn capture<F: FnOnce()>(f: F) -> Vec<String> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = log::set_boxed_logger(Box::new(CaptureLogger));
            log::set_max_level(log::LevelFilter::Warn);
        });
        WARNINGS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
        f();
        WARNINGS.with(|slot| slot.borrow_mut().take().unwrap_or_default())
    }

    fn timing_of(svg: &str, id: &str) -> SmilTiming {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let all: Vec<(NodeId, SvgNode)> = doc
            .descendants()
            .filter(|node| node.tag_name().map(|t| t.is_animation()).unwrap_or(false))
            .enumerate()
            .map(|(i, node)| (NodeId::from(i), node))
            .collect();
        let node = all
            .iter()
            .find(|(_, node)| node.element_id() == id)
            .map(|(_, node)| *node)
            .expect("animation id not found");
        parse_smil_timing(node, &all)
    }

    fn easing_of(svg: &str, id: &str, values_count: usize) -> Option<Easing> {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let node = doc
            .descendants()
            .find(|node| node.element_id() == id)
            .expect("id not found");
        parse_easing(node, values_count)
    }

    fn offsets(begins: &[Begin]) -> Vec<f32> {
        begins.iter().filter_map(begin_instant).collect()
    }

    #[test]
    fn clock_values() {
        assert_eq!(parse_clock_value("4"), Some(4.0));
        assert_eq!(parse_clock_value("3s"), Some(3.0));
        assert_eq!(parse_clock_value("1.5s"), Some(1.5));
        assert_eq!(parse_clock_value("02:30"), Some(150.0));
        assert_eq!(parse_clock_value("1min"), Some(60.0));
        assert_eq!(parse_clock_value("500ms"), Some(0.5));
        assert_eq!(parse_clock_value("01:00:00"), Some(3600.0));
        assert_eq!(parse_clock_value("bogus"), None);
    }

    #[test]
    fn multi_entry_begin_list() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s;4s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(offsets(timing.begins()), vec![0.0, 2.0, 4.0]);
    }

    #[test]
    fn omitted_begin_and_dur() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect><animate id='a' attributeName='opacity'/></rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(matches!(timing.begins(), [Begin::Offset(v)] if *v == 0.0));
        assert!(matches!(timing.dur(), Dur::Indefinite));
    }

    #[test]
    fn all_invalid_begin_is_empty() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='click;foo' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(timing.begins().is_empty());
    }

    #[test]
    fn event_begin_is_dropped_with_warning() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='click' dur='1s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let timing = timing_of(&svg, "a");
            assert!(timing.begins().is_empty());
        });
        assert!(warnings.contains(&"Unsupported animation begin/end value: 'click'.".to_string()));
    }

    #[test]
    fn min_max_are_dropped_with_warning() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' min='1s' max='5s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let _ = timing_of(&svg, "a");
        });
        assert!(warnings.contains(&"Unsupported SMIL timing attribute: 'min'.".to_string()));
        assert!(warnings.contains(&"Unsupported SMIL timing attribute: 'max'.".to_string()));
    }

    #[test]
    fn syncbase_chain_resolves() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='1s' dur='1s'/>\
             <animate id='b' attributeName='opacity' begin='a.begin+2s' dur='1s'/>\
             <animate id='c' attributeName='opacity' begin='b.begin+3s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "c");
        assert_eq!(offsets(timing.begins()), vec![6.0]);
    }

    #[test]
    fn end_reference_to_indefinite_repeat_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' repeatCount='indefinite'/>\
             <animate id='b' attributeName='opacity' begin='a.end+1s' dur='1s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let timing = timing_of(&svg, "b");
            assert!(timing.begins().is_empty());
        });
        assert!(
            warnings.contains(&"Unsupported animation begin/end value: 'a.end+1s'.".to_string())
        );
    }

    #[test]
    fn end_reference_to_multiple_begins_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s' dur='1s'/>\
             <animate id='b' attributeName='opacity' begin='a.end+1s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "b");
        assert!(timing.begins().is_empty());
    }

    #[test]
    fn end_reference_to_explicit_end_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' end='5s'/>\
             <animate id='b' attributeName='opacity' begin='a.end+1s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "b");
        assert!(timing.begins().is_empty());
    }

    #[test]
    fn end_list_selection() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;3s' end='2s;5s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].begin(), 0.0);
        assert_eq!(intervals[0].end(), Some(2.0));
        assert_eq!(intervals[1].begin(), 3.0);
        assert_eq!(intervals[1].end(), Some(5.0));
    }

    #[test]
    fn end_before_begin_is_skipped() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='5s' end='2s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(timing.intervals().is_empty());
    }

    #[test]
    fn identical_begins_collapse() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='1s;1s' dur='2s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(timing.intervals().len(), 1);
        assert_eq!(timing.intervals()[0].begin(), 1.0);
    }

    #[test]
    fn restart_never_keeps_first_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s' dur='1s' restart='never'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].begin(), 0.0);
        assert_eq!(intervals[0].end(), Some(1.0));
    }

    #[test]
    fn restart_when_not_active_accepts_after_end() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s' dur='10s' end='1s' restart='whenNotActive'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].begin(), 0.0);
        assert_eq!(intervals[0].end(), Some(1.0));
        assert_eq!(intervals[1].begin(), 2.0);
        assert_eq!(intervals[1].end(), Some(12.0));
    }

    #[test]
    fn zero_length_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='1s' end='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].begin(), 1.0);
        assert_eq!(intervals[0].end(), Some(1.0));
    }

    #[test]
    fn zero_duration_set_has_a_zero_length_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect><set id='a' attributeName='opacity' to='0.5' begin='1s' dur='0s' fill='freeze'/></rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(matches!(timing.dur(), Dur::Seconds(seconds) if *seconds == 0.0));
        assert_eq!(timing.intervals().len(), 1);
        assert_eq!(timing.intervals()[0].begin(), 1.0);
        assert_eq!(timing.intervals()[0].end(), Some(1.0));
    }

    #[test]
    fn cyclic_syncbase_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='b.begin' dur='1s'/>\
             <animate id='b' attributeName='opacity' begin='a.begin' dur='1s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let timing = timing_of(&svg, "a");
            assert!(timing.begins().is_empty());
        });
        assert!(
            warnings.contains(&"Unsupported animation begin/end value: 'b.begin'.".to_string())
        );
    }

    #[test]
    fn indefinite_duration_open_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect><animate id='a' attributeName='opacity' begin='0s'/></rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].begin(), 0.0);
        assert_eq!(intervals[0].end(), None);
    }

    #[test]
    fn repeat_count_bounds_active_duration() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='2s' repeatCount='3'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].begin(), 0.0);
        assert_eq!(intervals[0].end(), Some(6.0));
    }

    #[test]
    fn fill_and_restart_defaults() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(matches!(timing.fill(), SmilFill::Remove));
        assert!(matches!(timing.restart(), Restart::Always));
    }

    #[test]
    fn fill_freeze_parsed() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' fill='freeze'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(matches!(timing.fill(), SmilFill::Freeze));
    }

    #[test]
    fn easing_linear_key_times_valid() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;0.5;1'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 3).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Linear));
        assert_eq!(easing.key_times().unwrap().len(), 3);
    }

    #[test]
    fn easing_key_times_first_not_zero_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0.1;1'/>\
             </rect></svg>"
        );
        let warnings = capture(|| assert!(easing_of(&svg, "a", 2).is_none()));
        assert!(warnings.contains(&"Invalid animation timing: '0.1;1'.".to_string()));
    }

    #[test]
    fn easing_key_times_last_not_one_rejected_for_linear() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;0.5'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 2).is_none());
    }

    #[test]
    fn easing_key_times_count_mismatch_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 3).is_none());
    }

    #[test]
    fn easing_key_times_non_monotonic_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;0.8;0.5;1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 4).is_none());
    }

    #[test]
    fn easing_spline_valid() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;1' keySplines='0 0 1 1'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 2).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Spline));
        assert_eq!(easing.key_splines().unwrap().len(), 1);
    }

    #[test]
    fn easing_spline_out_of_range_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;1' keySplines='1.2 0 0 1'/>\
             </rect></svg>"
        );
        let warnings = capture(|| assert!(easing_of(&svg, "a", 2).is_none()));
        assert!(warnings.contains(&"Invalid animation timing: '1.2 0 0 1'.".to_string()));
    }

    #[test]
    fn easing_spline_count_mismatch_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;0.5;1' keySplines='0 0 1 1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 3).is_none());
    }

    #[test]
    fn easing_spline_missing_splines_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 2).is_none());
    }

    #[test]
    fn easing_discrete_waives_last_one() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='discrete' keyTimes='0;0.5'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 2).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Discrete));
        assert_eq!(easing.key_times().unwrap().len(), 2);
    }

    #[test]
    fn easing_discrete_first_not_zero_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='discrete' keyTimes='0.2;0.5'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 2).is_none());
    }

    #[test]
    fn easing_paced_ignores_key_times_and_splines() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='paced' keyTimes='bogus' keySplines='bogus'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 2).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Paced));
        assert!(easing.key_times().is_none());
        assert!(easing.key_splines().is_none());
    }
}
