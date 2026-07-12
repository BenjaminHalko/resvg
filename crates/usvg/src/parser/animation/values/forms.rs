// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::too_many_arguments)]

use crate::NormalizedF32;
use crate::tree::animation::{Accumulate, Additive, Keyframe};

use super::attributes::{warn_invalid_value, warn_unsupported_accumulate};

/// The raw string values of the four SMIL value forms.
pub(super) struct Forms<'a> {
    pub(super) values: Option<&'a str>,
    pub(super) from: Option<&'a str>,
    pub(super) to: Option<&'a str>,
    pub(super) by: Option<&'a str>,
}

/// Builds a keyframe track from the SMIL value forms and returns the resolved
/// additive behavior.
///
/// * `is_geometry` bakes a bare `by` against `base` instead of a `Sum` delta.
/// * `supports_delta` gates the `by` forms for non-interpolable types.
/// * `sum_zero` is the additive identity used for a bare `by` `Sum` delta.
pub(super) fn build_forms<T, P, A>(
    forms: &Forms,
    key_times: Option<&[NormalizedF32]>,
    additive: Additive,
    is_geometry: bool,
    supports_delta: bool,
    sum_zero: Option<T>,
    base: Option<T>,
    parse: P,
    delta_add: A,
) -> Option<(Vec<Keyframe<T>>, Additive)>
where
    T: Clone,
    P: Fn(&str) -> Option<T>,
    A: Fn(&T, &T) -> T,
{
    if let Some(values) = forms.values {
        return build_values_list(values, key_times, additive, parse);
    }

    let from = match forms.from {
        Some(s) => Some(parse(s.trim())?),
        None => None,
    };
    let to = match forms.to {
        Some(s) => Some(parse(s.trim())?),
        None => None,
    };
    let by = match forms.by {
        Some(s) => Some(parse(s.trim())?),
        None => None,
    };

    match (from, to, by) {
        (Some(f), Some(t), None) => Some((two_keyframes(f, t), additive)),
        (Some(f), None, Some(b)) => {
            if !supports_delta {
                warn_invalid_value(forms.by.unwrap_or_default().trim());
                return None;
            }
            let end = delta_add(&f, &b);
            Some((two_keyframes(f, end), Additive::Replace))
        }
        (None, Some(t), None) => {
            let base = base?;
            Some((two_keyframes(base, t), Additive::Replace))
        }
        (None, None, Some(b)) => {
            if !supports_delta {
                warn_invalid_value(forms.by.unwrap_or_default().trim());
                return None;
            }
            if is_geometry {
                let base = base?;
                let end = delta_add(&base, &b);
                Some((two_keyframes(base, end), Additive::Replace))
            } else {
                let zero = sum_zero?;
                Some((two_keyframes(zero, b), Additive::Sum))
            }
        }
        (Some(f), None, None) => {
            Some((vec![Keyframe::new(NormalizedF32::ZERO, f, None)], additive))
        }
        _ => None,
    }
}

/// Builds a keyframe track from a `values` list, dropping invalid entries.
fn build_values_list<T, P>(
    values: &str,
    key_times: Option<&[NormalizedF32]>,
    additive: Additive,
    parse: P,
) -> Option<(Vec<Keyframe<T>>, Additive)>
where
    T: Clone,
    P: Fn(&str) -> Option<T>,
{
    let raw: Vec<&str> = values
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if raw.is_empty() {
        return None;
    }

    let offsets = uniform_offsets(raw.len(), key_times);
    let mut keyframes = Vec::new();
    for (offset, item) in offsets.iter().zip(raw.iter()) {
        if let Some(value) = parse(item) {
            keyframes.push(Keyframe::new(*offset, value, None));
        }
    }

    (!keyframes.is_empty()).then_some((keyframes, additive))
}

/// Builds the two-keyframe track shared by the `from`/`to` and `by` forms.
fn two_keyframes<T: Clone>(start: T, end: T) -> Vec<Keyframe<T>> {
    vec![
        Keyframe::new(NormalizedF32::ZERO, start, None),
        Keyframe::new(NormalizedF32::ONE, end, None),
    ]
}

/// Computes keyframe offsets, honoring `keyTimes` when it matches the count.
fn uniform_offsets(count: usize, key_times: Option<&[NormalizedF32]>) -> Vec<NormalizedF32> {
    if let Some(times) = key_times {
        if times.len() == count {
            return times.to_vec();
        }
    }

    if count <= 1 {
        return vec![NormalizedF32::ZERO];
    }

    (0..count)
        .map(|i| NormalizedF32::new_clamped(i as f32 / (count as f32 - 1.0)))
        .collect()
}

/// Drops `Sum` accumulation for types that cannot accumulate.
pub(super) fn resolve_accumulate(accumulate: Accumulate, accumulatable: bool) -> Accumulate {
    if !accumulatable && matches!(accumulate, Accumulate::Sum) {
        warn_unsupported_accumulate();
        Accumulate::None
    } else {
        accumulate
    }
}

/// Moves a discrete `from`/`to` transition to the middle of its active interval.
pub(super) fn discrete_from_to_midpoint<T: Clone>(
    mut keyframes: Vec<Keyframe<T>>,
    forms: &Forms<'_>,
) -> Vec<Keyframe<T>> {
    if forms.values.is_none() && forms.from.is_some() && forms.to.is_some() && keyframes.len() == 2
    {
        let second = &keyframes[1];
        keyframes[1] = Keyframe::new(
            NormalizedF32::new_clamped(0.5),
            second.value().clone(),
            second.timing_function().cloned(),
        );
    }
    keyframes
}
