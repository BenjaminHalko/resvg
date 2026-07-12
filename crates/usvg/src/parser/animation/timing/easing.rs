// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::clock::parse_number;
use crate::NormalizedF32;
use crate::parser::svgtree::{AId, EId, SvgNode};
use crate::tree::animation::{CalcMode, Easing};

/// Parses `calcMode`, `keyTimes` and `keySplines` into an [`Easing`].
///
/// `values_count` is the number of animation values, used to validate the
/// `keyTimes`/`keySplines` counts. Invalid timing is dropped (returns `None`)
/// with a warning. `calcMode=paced` ignores `keyTimes` and `keySplines`.
pub(crate) fn parse_easing<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
    values_count: usize,
) -> Option<Easing> {
    let calc_mode = parse_calc_mode(node);

    if matches!(calc_mode, CalcMode::Paced) {
        return Some(Easing::new(CalcMode::Paced, None, None));
    }

    let key_times = match node.attribute::<&str>(AId::KeyTimes) {
        Some(raw) => match parse_key_times(raw) {
            Some(times) if valid_key_times(&times, calc_mode, values_count) => Some(times),
            _ => {
                log::warn!("Invalid animation timing: '{}'.", raw);
                return None;
            }
        },
        None => None,
    };

    let key_splines = if matches!(calc_mode, CalcMode::Spline) {
        let raw = node.attribute::<&str>(AId::KeySplines).unwrap_or("");
        let expected = match &key_times {
            Some(times) => times.len().saturating_sub(1),
            None => values_count.saturating_sub(1),
        };
        match parse_key_splines(raw) {
            Some(splines) if splines.len() == expected && splines_in_range(&splines) => {
                Some(splines)
            }
            _ => {
                log::warn!("Invalid animation timing: '{}'.", raw);
                return None;
            }
        }
    } else {
        None
    };

    let key_times = key_times.map(|times| {
        times
            .into_iter()
            .map(NormalizedF32::new_clamped)
            .collect::<Vec<_>>()
    });

    Some(Easing::new(calc_mode, key_times, key_splines))
}

/// Parses `calcMode`, defaulting to `paced` for `<animateMotion>` and `linear`
/// otherwise.
fn parse_calc_mode<'a, 'input>(node: SvgNode<'a, 'input>) -> CalcMode {
    match node.attribute::<&str>(AId::CalcMode) {
        Some("discrete") => CalcMode::Discrete,
        Some("linear") => CalcMode::Linear,
        Some("paced") => CalcMode::Paced,
        Some("spline") => CalcMode::Spline,
        _ => {
            if node.tag_name() == Some(EId::AnimateMotion) {
                CalcMode::Paced
            } else {
                CalcMode::Linear
            }
        }
    }
}

/// Parses a `keyTimes` value into raw numbers.
fn parse_key_times(value: &str) -> Option<Vec<f32>> {
    let mut times = Vec::new();
    for part in value.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        times.push(parse_number(part)?);
    }
    (!times.is_empty()).then_some(times)
}

/// Parses a `keySplines` value into cubic Bézier control point quadruples.
fn parse_key_splines(value: &str) -> Option<Vec<[f32; 4]>> {
    let mut splines = Vec::new();
    for group in value.split(';') {
        let group = group.trim();
        if group.is_empty() {
            continue;
        }

        let mut spline = [0.0f32; 4];
        let mut count = 0;
        for token in group.split([',', ' ', '\t', '\n', '\r']) {
            if token.is_empty() {
                continue;
            }
            if count == 4 {
                return None;
            }
            spline[count] = parse_number(token)?;
            count += 1;
        }

        if count != 4 {
            return None;
        }
        splines.push(spline);
    }
    (!splines.is_empty()).then_some(splines)
}

/// Validates `keyTimes` against the effective `calcMode`.
///
/// All modes require finite values in `[0, 1]`, a leading `0`, a monotonically
/// non-decreasing sequence and a count equal to the number of values. `discrete`
/// waives the trailing `1` requirement.
fn valid_key_times(times: &[f32], calc_mode: CalcMode, values_count: usize) -> bool {
    if times.len() != values_count {
        return false;
    }
    if times.first() != Some(&0.0) {
        return false;
    }
    if times.iter().any(|t| !(0.0f32..=1.0).contains(t)) {
        return false;
    }
    if times.windows(2).any(|w| w[0] > w[1]) {
        return false;
    }
    if !matches!(calc_mode, CalcMode::Discrete) && times.last() != Some(&1.0) {
        return false;
    }
    true
}

/// Checks that every `keySplines` control value is within `[0, 1]`.
fn splines_in_range(splines: &[[f32; 4]]) -> bool {
    splines.iter().flatten().all(|v| (0.0f32..=1.0).contains(v))
}
