// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use crate::tree::animation::{AnimationKind, Track};
use crate::{NonZeroRect, NormalizedF32};

use super::SmilValues;
use super::attributes::{AttributeContext, warned, warned_geometry};
use super::forms::{Forms, build_forms};
use super::opacity::parse_opacity;

pub(super) fn parse_offset_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        true,
        Some(NormalizedF32::ZERO),
        None,
        |s| warned(parse_offset(s), s),
        |a, b| NormalizedF32::new_clamped(a.get() + b.get()),
    )?;
    Some(SmilValues {
        kind: AnimationKind::StopOffset(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_geometry_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
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
        true,
        true,
        Some(0.0f32),
        base_value.number(),
        |s| warned_geometry(parse_geometry_number(s), s),
        |a, b| a + b,
    )?;
    Some(SmilValues {
        kind: AnimationKind::Geometry(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

/// Parses geometry forms after their lengths have been resolved into user units.
pub(crate) fn parse_resolved_geometry_values(
    forms: &Forms<'_>,
    key_times: Option<&[NormalizedF32]>,
    additive: crate::tree::animation::Additive,
    accumulate: crate::tree::animation::Accumulate,
    calc_mode: crate::tree::animation::CalcMode,
    base_value: Option<f32>,
    resolve: impl Fn(&str) -> Option<f32>,
) -> Option<SmilValues> {
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        true,
        Some(0.0),
        base_value,
        |value| warned_geometry(resolve(value), value),
        |a, b| a + b,
    )?;
    Some(SmilValues {
        kind: AnimationKind::Geometry(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_view_box_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        true,
        true,
        None,
        None,
        |s| warned(parse_rect(s), s),
        add_rects,
    )?;
    Some(SmilValues {
        kind: AnimationKind::ViewBox(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

fn parse_offset(value: &str) -> Option<NormalizedF32> {
    parse_opacity(value)
}

fn parse_geometry_number(value: &str) -> Option<f32> {
    svgtypes::Length::from_str(value)
        .ok()
        .map(|l| l.number as f32)
}

fn parse_rect(value: &str) -> Option<NonZeroRect> {
    let vb = svgtypes::ViewBox::from_str(value).ok()?;
    NonZeroRect::from_xywh(vb.x as f32, vb.y as f32, vb.w as f32, vb.h as f32)
}

/// Adds two rects component-wise, falling back to `a` on a degenerate result.
fn add_rects(a: &NonZeroRect, b: &NonZeroRect) -> NonZeroRect {
    NonZeroRect::from_xywh(
        a.x() + b.x(),
        a.y() + b.y(),
        a.width() + b.width(),
        a.height() + b.height(),
    )
    .unwrap_or(*a)
}
