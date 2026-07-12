// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use crate::Opacity;

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

pub(super) fn parse_css_number(value: &str) -> Option<f32> {
    let length = svgtypes::Length::from_str(value).ok()?;
    length.number.is_finite().then_some(length.number as f32)
}
