// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use crate::Opacity;
use crate::parser::converter;
use crate::parser::svgtree::{AId, SvgNode};
use crate::parser::units;

pub(super) fn parse_css_opacity(value: &str) -> Option<Opacity> {
    let length = svgtypes::Length::from_str(value).ok()?;
    match length.unit {
        svgtypes::LengthUnit::Percent => Some(Opacity::new_clamped(length.number as f32 / 100.0)),
        svgtypes::LengthUnit::None => Some(Opacity::new_clamped(length.number as f32)),
        _ => None,
    }
}

pub(super) fn parse_css_color(value: &str) -> Option<svgtypes::Color> {
    svgtypes::Color::from_str(value).ok()
}

fn parse_css_length(value: &str, node: SvgNode, aid: AId, state: &converter::State) -> Option<f32> {
    let length = svgtypes::Length::from_str(value).ok()?;
    let resolved = units::convert_user_length(length, node, aid, state);
    resolved.is_finite().then_some(resolved)
}

pub(super) fn parse_css_stroke_width(
    value: &str,
    node: SvgNode,
    state: &converter::State,
) -> Option<f32> {
    let resolved = parse_css_length(value, node, AId::StrokeWidth, state)?;
    (resolved >= 0.0).then_some(resolved)
}

pub(super) fn parse_css_stroke_dashoffset(
    value: &str,
    node: SvgNode,
    state: &converter::State,
) -> Option<f32> {
    parse_css_length(value, node, AId::StrokeDashoffset, state)
}
