// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Parses an optionally-signed clock value offset.
pub(super) fn parse_offset(value: &str) -> Option<f32> {
    let value = value.trim();
    let (sign, rest) = if let Some(rest) = value.strip_prefix('+') {
        (1.0, rest)
    } else if let Some(rest) = value.strip_prefix('-') {
        (-1.0, rest)
    } else {
        (1.0, value)
    };
    parse_clock_value(rest.trim()).map(|seconds| sign * seconds)
}

/// Parses a SMIL clock value (e.g. `4`, `3s`, `1.5s`, `02:30`, `1min`, `500ms`)
/// into seconds.
pub(super) fn parse_clock_value(value: &str) -> Option<f32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value.contains(':') {
        return parse_clock_colon(value);
    }

    if let Some(number) = value.strip_suffix("ms") {
        return parse_number(number).map(|v| v / 1000.0);
    }
    if let Some(number) = value.strip_suffix("min") {
        return parse_number(number).map(|v| v * 60.0);
    }
    if let Some(number) = value.strip_suffix('h') {
        return parse_number(number).map(|v| v * 3600.0);
    }
    if let Some(number) = value.strip_suffix('s') {
        return parse_number(number);
    }

    parse_number(value)
}

/// Parses the `HH:MM:SS(.fff)` and `MM:SS(.fff)` clock forms.
fn parse_clock_colon(value: &str) -> Option<f32> {
    let mut parts = value.split(':');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }

    let (hours, minutes, seconds) = match third {
        Some(third) => (
            parse_number(first)?,
            parse_number(second)?,
            parse_number(third)?,
        ),
        None => (0.0, parse_number(first)?, parse_number(second)?),
    };

    if hours < 0.0 || minutes < 0.0 || seconds < 0.0 {
        return None;
    }

    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Parses a finite `f32`.
pub(super) fn parse_number(value: &str) -> Option<f32> {
    let number: f32 = value.trim().parse().ok()?;
    number.is_finite().then_some(number)
}
