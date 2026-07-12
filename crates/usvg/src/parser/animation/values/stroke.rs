// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use crate::StrokeMiterlimit;
use crate::tree::animation::{AnimationKind, CalcMode, Keyframe, Track};

use super::SmilValues;
use super::attributes::{AttributeContext, warn_not_interpolable, warned};
use super::forms::build_forms;

pub(super) fn parse_stroke_width_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        base_value,
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        true,
        Some(0.0f32),
        base_value.number(),
        |s| warned(parse_nonneg_number(s), s),
        |a, b| a + b,
    )?;
    Some(SmilValues {
        kind: AnimationKind::StrokeWidth(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_stroke_dashoffset_attribute(
    context: AttributeContext<'_>,
) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        base_value,
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        true,
        Some(0.0f32),
        base_value.number(),
        |s| warned(parse_number(s), s),
        |a, b| a + b,
    )?;
    Some(SmilValues {
        kind: AnimationKind::StrokeDashoffset(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_stroke_dasharray_attribute(
    context: AttributeContext<'_>,
) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        base_value,
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        true,
        None,
        base_value.numbers(),
        |s| warned(parse_number_list(s), s),
        |a, b| {
            let len = a.len().max(b.len());
            (0..len)
                .map(|i| a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0))
                .collect()
        },
    )?;
    let calc_mode = dasharray_calc_mode(&keyframes, calc_mode);
    Some(SmilValues {
        kind: AnimationKind::StrokeDasharray(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_stroke_miterlimit_attribute(
    context: AttributeContext<'_>,
) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        base_value,
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        true,
        None,
        base_value.miterlimit(),
        |s| warned(parse_miterlimit(s), s),
        |a, b| StrokeMiterlimit::new(a.get() + b.get()),
    )?;
    Some(SmilValues {
        kind: AnimationKind::StrokeMiterlimit(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

fn parse_number(value: &str) -> Option<f32> {
    svgtypes::Number::from_str(value).ok().map(|n| n.0 as f32)
}

fn parse_nonneg_number(value: &str) -> Option<f32> {
    let number = parse_number(value)?;
    (number >= 0.0).then_some(number)
}

fn parse_miterlimit(value: &str) -> Option<StrokeMiterlimit> {
    parse_number(value).map(StrokeMiterlimit::new)
}

pub(super) fn parse_number_list(value: &str) -> Option<Vec<f32>> {
    let mut list = Vec::new();
    for number in svgtypes::NumberListParser::from(value) {
        list.push(number.ok()? as f32);
    }
    (!list.is_empty()).then_some(list)
}

/// Forces discrete stepping when the dash-array keyframes differ in length.
fn dasharray_calc_mode(keyframes: &[Keyframe<Vec<f32>>], calc_mode: CalcMode) -> CalcMode {
    let Some(first) = keyframes.first() else {
        return calc_mode;
    };

    let len = first.value().len();
    if keyframes.iter().any(|k| k.value().len() != len) {
        warn_not_interpolable();
        CalcMode::Discrete
    } else {
        calc_mode
    }
}
