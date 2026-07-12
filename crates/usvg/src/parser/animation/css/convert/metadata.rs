// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::parser::svgtree::{AId, SvgNode};
use crate::tree::animation::{
    CssFillMode, Direction, Iterations, PlayState, TransformBox, TransformOrigin,
    TransformOriginValue,
};

use super::super::scanner::split_top_level;
use super::timing::{parse_finite, strip_suffix_ci};

pub(super) fn read_transform_origin(node: SvgNode) -> TransformOrigin {
    match node.try_attribute::<svgtypes::TransformOrigin>(AId::TransformOrigin) {
        Some(origin) => TransformOrigin::new(
            origin_component(origin.x_offset),
            origin_component(origin.y_offset),
        ),
        None => TransformOrigin::new(
            TransformOriginValue::Percent(50.0),
            TransformOriginValue::Percent(50.0),
        ),
    }
}

fn origin_component(length: svgtypes::Length) -> TransformOriginValue {
    if length.unit == svgtypes::LengthUnit::Percent {
        TransformOriginValue::Percent(length.number as f32)
    } else {
        TransformOriginValue::Length(length.number as f32)
    }
}

pub(super) fn read_transform_box(node: SvgNode) -> TransformBox {
    match node.try_attribute::<&str>(AId::TransformBox).map(str::trim) {
        Some("content-box") => TransformBox::ContentBox,
        Some("border-box") => TransformBox::BorderBox,
        Some("fill-box") => TransformBox::FillBox,
        Some("stroke-box") => TransformBox::StrokeBox,
        _ => TransformBox::ViewBox,
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

pub(super) fn parse_iterations(value: &str) -> Iterations {
    if value.trim().eq_ignore_ascii_case("infinite") {
        return Iterations::Infinite;
    }
    match parse_finite(value) {
        Some(count) if count >= 0.0 => Iterations::Count(count),
        _ => Iterations::Count(1.0),
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

pub(super) fn parse_fill_mode(value: &str) -> CssFillMode {
    let value = value.trim();
    if value.eq_ignore_ascii_case("forwards") {
        CssFillMode::Forwards
    } else if value.eq_ignore_ascii_case("backwards") {
        CssFillMode::Backwards
    } else if value.eq_ignore_ascii_case("both") {
        CssFillMode::Both
    } else {
        CssFillMode::None
    }
}

pub(super) fn parse_play_state(value: &str) -> PlayState {
    if value.trim().eq_ignore_ascii_case("paused") {
        PlayState::Paused
    } else {
        PlayState::Running
    }
}
