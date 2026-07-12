// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tree::animation::{AnimationKind, AnimationVisibility, CalcMode, Track};
use crate::{FillRule, LineCap, LineJoin};

use super::SmilValues;
use super::attributes::{AttributeContext, warned};
use super::forms::{build_forms, discrete_from_to_midpoint, resolve_accumulate};

pub(super) fn parse_stroke_linecap_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        base_value,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        false,
        None,
        base_value.linecap(),
        |s| warned(parse_linecap(s), s),
        |a, _| *a,
    )?;
    Some(SmilValues {
        kind: AnimationKind::StrokeLinecap(Track::new(keyframes)),
        additive,
        accumulate: resolve_accumulate(accumulate, false),
        calc_mode: CalcMode::Discrete,
    })
}

pub(super) fn parse_stroke_linejoin_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        base_value,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        false,
        None,
        base_value.linejoin(),
        |s| warned(parse_linejoin(s), s),
        |a, _| *a,
    )?;
    Some(SmilValues {
        kind: AnimationKind::StrokeLinejoin(Track::new(keyframes)),
        additive,
        accumulate: resolve_accumulate(accumulate, false),
        calc_mode: CalcMode::Discrete,
    })
}

pub(super) fn parse_fill_rule_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        base_value,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        false,
        None,
        base_value.fill_rule(),
        |s| warned(parse_fill_rule(s), s),
        |a, _| *a,
    )?;
    Some(SmilValues {
        kind: AnimationKind::FillRule(Track::new(keyframes)),
        additive,
        accumulate: resolve_accumulate(accumulate, false),
        calc_mode: CalcMode::Discrete,
    })
}

pub(super) fn parse_display_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        base_value,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        false,
        None,
        base_value.boolean(),
        |s| Some(parse_display(s, base_value.boolean().unwrap_or(true))),
        |a, _| *a,
    )?;
    let keyframes = discrete_from_to_midpoint(keyframes, forms);
    Some(SmilValues {
        kind: AnimationKind::Display(Track::new(keyframes)),
        additive,
        accumulate: resolve_accumulate(accumulate, false),
        calc_mode: CalcMode::Discrete,
    })
}

pub(super) fn parse_visibility_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
    let AttributeContext {
        forms,
        key_times,
        additive,
        accumulate,
        base_value,
        ..
    } = context;
    let (keyframes, additive) = build_forms(
        forms,
        key_times,
        additive,
        false,
        false,
        None,
        base_value.visibility(),
        |s| {
            warned(
                parse_visibility(
                    s,
                    base_value
                        .visibility()
                        .unwrap_or(AnimationVisibility::Visible),
                ),
                s,
            )
        },
        |a, _| *a,
    )?;
    let keyframes = discrete_from_to_midpoint(keyframes, forms);
    Some(SmilValues {
        kind: AnimationKind::Visibility(Track::new(keyframes)),
        additive,
        accumulate: resolve_accumulate(accumulate, false),
        calc_mode: CalcMode::Discrete,
    })
}

fn parse_linecap(value: &str) -> Option<LineCap> {
    match value {
        "butt" => Some(LineCap::Butt),
        "round" => Some(LineCap::Round),
        "square" => Some(LineCap::Square),
        _ => None,
    }
}

fn parse_linejoin(value: &str) -> Option<LineJoin> {
    match value {
        "miter" => Some(LineJoin::Miter),
        "miter-clip" => Some(LineJoin::MiterClip),
        "round" => Some(LineJoin::Round),
        "bevel" => Some(LineJoin::Bevel),
        _ => None,
    }
}

fn parse_fill_rule(value: &str) -> Option<FillRule> {
    match value {
        "nonzero" => Some(FillRule::NonZero),
        "evenodd" => Some(FillRule::EvenOdd),
        _ => None,
    }
}

fn parse_display(value: &str, inherited: bool) -> bool {
    match value {
        "none" => false,
        "inherit" => inherited,
        _ => true,
    }
}

fn parse_visibility(value: &str, inherited: AnimationVisibility) -> Option<AnimationVisibility> {
    match value {
        "visible" => Some(AnimationVisibility::Visible),
        "inherit" => Some(inherited),
        "hidden" => Some(AnimationVisibility::Hidden),
        "collapse" => Some(AnimationVisibility::Collapse),
        _ => None,
    }
}
