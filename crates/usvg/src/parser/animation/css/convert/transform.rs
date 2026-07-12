// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use crate::tree::animation::TransformFunction;

use super::timing::{parse_finite, strip_suffix_ci};

/// Parses a CSS `transform` value into a list of transform functions.
pub(super) fn parse_transform_functions(value: &str) -> Option<Vec<TransformFunction>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }

    let mut functions = Vec::new();
    let mut rest = value;
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let name = rest[..open].trim();
        let after = &rest[open + 1..];
        let close = after.find(')')?;
        functions.push(parse_transform_function(name, &after[..close])?);
        rest = after[close + 1..].trim_start();
    }

    (!functions.is_empty()).then_some(functions)
}

fn parse_transform_function(name: &str, arguments: &str) -> Option<TransformFunction> {
    let arguments: Vec<&str> = arguments
        .split(',')
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
        .collect();

    let function = if name.eq_ignore_ascii_case("matrix") {
        if arguments.len() != 6 {
            return None;
        }
        let mut values = [0.0f32; 6];
        for (slot, argument) in values.iter_mut().zip(arguments.iter().copied()) {
            *slot = parse_finite(argument)?;
        }
        TransformFunction::Matrix(
            values[0], values[1], values[2], values[3], values[4], values[5],
        )
    } else if name.eq_ignore_ascii_case("translate") {
        let tx = parse_length(arguments.first()?)?;
        let ty = match arguments.get(1) {
            Some(value) => parse_length(value)?,
            None => 0.0,
        };
        (arguments.len() <= 2).then_some(TransformFunction::Translate(tx, ty))?
    } else if name.eq_ignore_ascii_case("translatex") {
        TransformFunction::TranslateX(parse_length(single(&arguments)?)?)
    } else if name.eq_ignore_ascii_case("translatey") {
        TransformFunction::TranslateY(parse_length(single(&arguments)?)?)
    } else if name.eq_ignore_ascii_case("scale") {
        let sx = parse_finite(arguments.first()?)?;
        let sy = match arguments.get(1) {
            Some(value) => parse_finite(value)?,
            None => sx,
        };
        (arguments.len() <= 2).then_some(TransformFunction::Scale(sx, sy))?
    } else if name.eq_ignore_ascii_case("scalex") {
        TransformFunction::ScaleX(parse_finite(single(&arguments)?)?)
    } else if name.eq_ignore_ascii_case("scaley") {
        TransformFunction::ScaleY(parse_finite(single(&arguments)?)?)
    } else if name.eq_ignore_ascii_case("rotate") {
        TransformFunction::Rotate(parse_angle(single(&arguments)?)?)
    } else if name.eq_ignore_ascii_case("skewx") {
        TransformFunction::SkewX(parse_angle(single(&arguments)?)?)
    } else if name.eq_ignore_ascii_case("skewy") {
        TransformFunction::SkewY(parse_angle(single(&arguments)?)?)
    } else {
        return None;
    };

    Some(function)
}

fn single<'a>(arguments: &[&'a str]) -> Option<&'a str> {
    match arguments {
        [argument] => Some(*argument),
        _ => None,
    }
}

/// Parses a CSS `<length>` used by transforms, accepting only user-unit values.
fn parse_length(value: &str) -> Option<f32> {
    let length = svgtypes::Length::from_str(value).ok()?;
    match length.unit {
        svgtypes::LengthUnit::None | svgtypes::LengthUnit::Px => {
            length.number.is_finite().then_some(length.number as f32)
        }
        _ => None,
    }
}

/// Parses a CSS `<angle>` into degrees.
fn parse_angle(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(number) = strip_suffix_ci(value, "deg") {
        return parse_finite(number);
    }
    if let Some(number) = strip_suffix_ci(value, "grad") {
        return parse_finite(number).map(|gradians| gradians * 0.9);
    }
    if let Some(number) = strip_suffix_ci(value, "rad") {
        return parse_finite(number).map(f32::to_degrees);
    }
    if let Some(number) = strip_suffix_ci(value, "turn") {
        return parse_finite(number).map(|turns| turns * 360.0);
    }
    parse_finite(value)
}
