// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::parser::converter;
use crate::parser::svgtree::{AId, SvgNode};
use crate::tree::animation::{
    CssBox, CssOrigin, Direction, Interval, OriginComponent, TimedInterval, Timing,
};

use super::super::scanner::split_top_level;
use super::timing::{parse_finite, strip_suffix_ci};

pub(super) fn read_transform_origin(node: SvgNode, state: &converter::State) -> CssOrigin {
    match node.try_attribute::<svgtypes::TransformOrigin>(AId::TransformOrigin) {
        Some(origin) => CssOrigin::new(
            origin_component(origin.x_offset, node, state),
            origin_component(origin.y_offset, node, state),
            read_transform_box(node),
        ),
        None => CssOrigin::new(
            OriginComponent::Percent(50.0),
            OriginComponent::Percent(50.0),
            read_transform_box(node),
        ),
    }
}

fn origin_component(
    length: svgtypes::Length,
    node: SvgNode,
    state: &converter::State,
) -> OriginComponent {
    if length.unit == svgtypes::LengthUnit::Percent {
        OriginComponent::Percent(length.number as f32)
    } else {
        OriginComponent::Length(node.convert_user_length(AId::TransformOrigin, state, length))
    }
}

pub(super) fn read_transform_box(node: SvgNode) -> CssBox {
    match node.try_attribute::<&str>(AId::TransformBox).map(str::trim) {
        Some("content-box") => CssBox::Content,
        Some("border-box") => CssBox::Border,
        Some("fill-box") => CssBox::Fill,
        Some("stroke-box") => CssBox::Stroke,
        _ => CssBox::View,
    }
}

pub(super) fn split_list(value: &str) -> Vec<&str> {
    // A single value such as `steps(4, jump-end)` may carry its own commas, so
    // the list is split at the top level only.
    split_top_level(value, b',')
        .into_iter()
        .map(str::trim)
        .collect()
}

pub(super) fn longhand_list<'a>(node: SvgNode<'a, '_>, aid: AId) -> Vec<&'a str> {
    node.attribute::<&str>(aid)
        .map(split_list)
        .unwrap_or_default()
}

/// Reads the `index`th list entry, cycling as CSS does when a longhand list is
/// shorter than the `animation-name` list.
pub(super) fn cycle<'a>(list: &[&'a str], index: usize) -> Option<&'a str> {
    if list.is_empty() {
        None
    } else {
        Some(list[index % list.len()])
    }
}

pub(super) fn parse_time(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(number) = strip_suffix_ci(value, "ms") {
        return parse_finite(number).map(|seconds| seconds / 1000.0);
    }
    if let Some(number) = strip_suffix_ci(value, "s") {
        return parse_finite(number);
    }
    parse_finite(value)
}

#[derive(Clone, Copy)]
pub(super) enum IterationLimit {
    Count(f32),
    Indefinite,
}

#[derive(Clone, Copy)]
pub(super) enum FillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

pub(super) fn parse_iterations(value: &str) -> IterationLimit {
    if value.trim().eq_ignore_ascii_case("infinite") {
        return IterationLimit::Indefinite;
    }
    match parse_finite(value) {
        Some(count) if count >= 0.0 => IterationLimit::Count(count),
        _ => IterationLimit::Count(1.0),
    }
}

pub(super) fn parse_direction(value: &str) -> Direction {
    let value = value.trim();
    if value.eq_ignore_ascii_case("reverse") {
        Direction::Reverse
    } else if value.eq_ignore_ascii_case("alternate") {
        Direction::Alternate
    } else if value.eq_ignore_ascii_case("alternate-reverse") {
        Direction::AlternateReverse
    } else {
        Direction::Normal
    }
}

pub(super) fn parse_fill_mode(value: &str) -> FillMode {
    let value = value.trim();
    if value.eq_ignore_ascii_case("forwards") {
        FillMode::Forwards
    } else if value.eq_ignore_ascii_case("backwards") {
        FillMode::Backwards
    } else if value.eq_ignore_ascii_case("both") {
        FillMode::Both
    } else {
        FillMode::None
    }
}

pub(super) fn is_paused(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("paused")
}

pub(super) fn bake_timing(
    duration: f32,
    delay: f32,
    limit: IterationLimit,
    direction: Direction,
    fill: FillMode,
    paused: bool,
) -> Timing {
    let iterations = match limit {
        IterationLimit::Count(count) => count.max(0.0),
        IterationLimit::Indefinite => f32::INFINITY,
    };
    let one_loop_end = Some(delay.max(0.0) + duration);
    if paused {
        return Timing::new(
            Vec::new(),
            (duration > 0.0).then_some(duration),
            direction,
            sample(duration, iterations, direction, fill, (-delay).max(0.0)),
            one_loop_end,
        );
    }

    let active_duration = duration * iterations;
    let interval = if duration > 0.0 && !active_duration.is_finite() {
        Interval::new(delay, None)
    } else {
        Interval::new_relative(delay, active_duration.max(0.0))
    };
    Timing::new(
        vec![TimedInterval::new(
            interval,
            terminal(fill, direction, iterations),
        )],
        (duration > 0.0).then_some(duration),
        direction,
        before(fill, direction),
        one_loop_end,
    )
}

fn sample(
    duration: f32,
    iterations: f32,
    direction: Direction,
    fill: FillMode,
    local_time: f32,
) -> Option<f32> {
    if local_time < 0.0 {
        return before(fill, direction);
    }
    let active_duration = duration * iterations;
    if duration > 0.0 && active_duration.is_finite() && local_time >= active_duration {
        return terminal(fill, direction, iterations);
    }
    if duration <= 0.0 {
        return terminal(fill, direction, iterations);
    }
    Some(directed_progress(local_time / duration, direction, false))
}

fn before(fill: FillMode, direction: Direction) -> Option<f32> {
    match fill {
        FillMode::Backwards | FillMode::Both => Some(directed_progress(0.0, direction, false)),
        FillMode::None | FillMode::Forwards => None,
    }
}

fn terminal(fill: FillMode, direction: Direction, iterations: f32) -> Option<f32> {
    match fill {
        FillMode::Forwards | FillMode::Both => Some(directed_progress(iterations, direction, true)),
        FillMode::None | FillMode::Backwards => None,
    }
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
