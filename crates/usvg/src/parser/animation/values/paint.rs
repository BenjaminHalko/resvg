// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tree::animation::{AnimationKind, Track};

use super::SmilValues;
use super::attributes::{AttributeContext, warn_invalid_value, warn_unsupported_paint};
use super::forms::build_forms;

pub(super) fn parse_fill_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
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
        false,
        None,
        base_value.color(),
        parse_color_form,
        |a, _| *a,
    )?;
    Some(SmilValues {
        kind: AnimationKind::Fill(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_stroke_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
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
        false,
        None,
        base_value.color(),
        parse_color_form,
        |a, _| *a,
    )?;
    Some(SmilValues {
        kind: AnimationKind::Stroke(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

pub(super) fn parse_stop_color_attribute(context: AttributeContext<'_>) -> Option<SmilValues> {
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
        false,
        None,
        base_value.color(),
        parse_color_form,
        |a, _| *a,
    )?;
    Some(SmilValues {
        kind: AnimationKind::StopColor(Track::new(keyframes)),
        additive,
        accumulate,
        calc_mode,
    })
}

/// The outcome of parsing a paint value as a solid color.
enum ColorForm {
    /// A solid color.
    Color(svgtypes::Color),
    /// A `url(#...)` paint reference, which cannot be animated as a color.
    Url,
    /// An unparsable value.
    Invalid,
}

fn parse_paint_color(value: &str) -> ColorForm {
    match svgtypes::Paint::from_str(value) {
        Ok(svgtypes::Paint::Color(color)) => ColorForm::Color(color),
        Ok(svgtypes::Paint::FuncIRI(..)) => ColorForm::Url,
        _ => ColorForm::Invalid,
    }
}

/// Parses a solid color, warning on `url(#...)` and invalid values.
fn parse_color_form(value: &str) -> Option<svgtypes::Color> {
    match parse_paint_color(value) {
        ColorForm::Color(color) => Some(color),
        ColorForm::Url => {
            warn_unsupported_paint(value);
            None
        }
        ColorForm::Invalid => {
            warn_invalid_value(value);
            None
        }
    }
}
