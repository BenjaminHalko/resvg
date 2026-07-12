// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tree::animation::{StepPosition, TimingFunction};

pub(super) fn parse_timing_function(value: &str) -> Option<TimingFunction> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("linear") {
        return Some(TimingFunction::Linear);
    }
    if value.eq_ignore_ascii_case("ease") {
        return Some(TimingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0));
    }
    if value.eq_ignore_ascii_case("ease-in") {
        return Some(TimingFunction::CubicBezier(0.42, 0.0, 1.0, 1.0));
    }
    if value.eq_ignore_ascii_case("ease-out") {
        return Some(TimingFunction::CubicBezier(0.0, 0.0, 0.58, 1.0));
    }
    if value.eq_ignore_ascii_case("ease-in-out") {
        return Some(TimingFunction::CubicBezier(0.42, 0.0, 0.58, 1.0));
    }
    if value.eq_ignore_ascii_case("step-start") {
        return Some(TimingFunction::Steps(1, StepPosition::JumpStart));
    }
    if value.eq_ignore_ascii_case("step-end") {
        return Some(TimingFunction::Steps(1, StepPosition::JumpEnd));
    }
    if let Some(arguments) = function_arguments(value, "steps") {
        return parse_steps(arguments);
    }
    if let Some(arguments) = function_arguments(value, "cubic-bezier") {
        return parse_cubic_bezier(arguments);
    }
    None
}

/// Returns the argument list of a `name(...)` functional value.
fn function_arguments<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let inner = value.strip_suffix(')')?;
    let open = inner.find('(')?;
    inner[..open]
        .trim()
        .eq_ignore_ascii_case(name)
        .then(|| &inner[open + 1..])
}

fn parse_steps(arguments: &str) -> Option<TimingFunction> {
    let mut parts = arguments.split(',');
    let count: u32 = parts.next()?.trim().parse().ok()?;
    if count == 0 {
        return None;
    }
    let position = match parts.next() {
        Some(keyword) => parse_step_position(keyword.trim())?,
        None => StepPosition::JumpEnd,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(TimingFunction::Steps(count, position))
}

fn parse_step_position(keyword: &str) -> Option<StepPosition> {
    match keyword {
        "jump-start" | "start" => Some(StepPosition::JumpStart),
        "jump-end" | "end" => Some(StepPosition::JumpEnd),
        "jump-none" => Some(StepPosition::JumpNone),
        "jump-both" => Some(StepPosition::JumpBoth),
        _ => None,
    }
}

fn parse_cubic_bezier(arguments: &str) -> Option<TimingFunction> {
    let mut values = [0.0f32; 4];
    let mut arguments = arguments.split(',');
    for value in &mut values {
        *value = parse_finite(arguments.next()?.trim())?;
    }
    arguments
        .next()
        .is_none()
        .then(|| TimingFunction::CubicBezier(values[0], values[1], values[2], values[3]))
}

pub(super) fn parse_finite(value: &str) -> Option<f32> {
    let number: f32 = value.trim().parse().ok()?;
    number.is_finite().then_some(number)
}

pub(super) fn strip_suffix_ci<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
    if value.len() < suffix.len() {
        return None;
    }
    let split = value.len() - suffix.len();
    let (head, tail) = value.split_at(split);
    tail.eq_ignore_ascii_case(suffix).then_some(head)
}
