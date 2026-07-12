// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use crate::Opacity;
use crate::tree::animation::{AnimationKind, Track};

use super::SmilValues;
use super::attributes::{AttributeContext, warned};
use super::forms::build_forms;

pub(super) fn parse_opacity_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
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
        Some(Opacity::ZERO),
        base_value.opacity(),
        |s| warned(parse_opacity(s), s),
        |a, b| Opacity::new_clamped(a.get() + b.get()),
    )?;
    Some(SmilValues {
        kind: AnimationKind::Opacity(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_stop_opacity_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
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
        Some(Opacity::ZERO),
        base_value.opacity(),
        |s| warned(parse_opacity(s), s),
        |a, b| Opacity::new_clamped(a.get() + b.get()),
    )?;
    Some(SmilValues {
        kind: AnimationKind::StopOpacity(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_opacity(value: &str) -> Option<Opacity> {
    let length = svgtypes::Length::from_str(value).ok()?;
    match length.unit {
        svgtypes::LengthUnit::Percent => Some(Opacity::new_clamped(length.number as f32 / 100.0)),
        svgtypes::LengthUnit::None => Some(Opacity::new_clamped(length.number as f32)),
        _ => None,
    }
}
