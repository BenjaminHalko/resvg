// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Expansion of the `animation` shorthand property.
//!
//! Implements the CSS Animations Level 1 grammar so a single `animation`
//! declaration can be rewritten into its eight longhands, index-matched across
//! comma-separated animation definitions.

/// The eight longhands the `animation` shorthand expands into.
///
/// Every field is a comma-separated list. The lists are index-matched: the Nth
/// entry of each belongs to the Nth animation in the shorthand.
pub(crate) struct AnimationLonghands {
    pub(crate) name: String,
    pub(crate) duration: String,
    pub(crate) timing_function: String,
    pub(crate) delay: String,
    pub(crate) iteration_count: String,
    pub(crate) direction: String,
    pub(crate) fill_mode: String,
    pub(crate) play_state: String,
}

/// Expands an `animation` shorthand value into its longhands.
///
/// Returns `None` when any animation in the comma-separated list is malformed,
/// matching the CSS rule that an invalid declaration is dropped as a whole.
pub(crate) fn expand_animation_shorthand(value: &str) -> Option<AnimationLonghands> {
    let mut names = Vec::new();
    let mut durations = Vec::new();
    let mut timing_functions = Vec::new();
    let mut delays = Vec::new();
    let mut iteration_counts = Vec::new();
    let mut directions = Vec::new();
    let mut fill_modes = Vec::new();
    let mut play_states = Vec::new();

    for part in split_top_level(value, b',') {
        let single = parse_single_animation(part.trim())?;
        names.push(single.name.unwrap_or_else(|| "none".to_string()));
        durations.push(single.duration.unwrap_or_else(|| "0s".to_string()));
        timing_functions.push(single.timing_function.unwrap_or_else(|| "ease".to_string()));
        delays.push(single.delay.unwrap_or_else(|| "0s".to_string()));
        iteration_counts.push(single.iteration_count.unwrap_or_else(|| "1".to_string()));
        directions.push(single.direction.unwrap_or_else(|| "normal".to_string()));
        fill_modes.push(single.fill_mode.unwrap_or_else(|| "none".to_string()));
        play_states.push(single.play_state.unwrap_or_else(|| "running".to_string()));
    }

    Some(AnimationLonghands {
        name: names.join(", "),
        duration: durations.join(", "),
        timing_function: timing_functions.join(", "),
        delay: delays.join(", "),
        iteration_count: iteration_counts.join(", "),
        direction: directions.join(", "),
        fill_mode: fill_modes.join(", "),
        play_state: play_states.join(", "),
    })
}

#[derive(Default)]
struct SingleAnimation {
    duration: Option<String>,
    delay: Option<String>,
    timing_function: Option<String>,
    iteration_count: Option<String>,
    direction: Option<String>,
    fill_mode: Option<String>,
    play_state: Option<String>,
    name: Option<String>,
    name_seen: bool,
}

/// Parses one space-separated animation definition.
///
/// Returns `None` when a value cannot be assigned to a longhand, or when a
/// longhand is specified more than once (e.g. a third `<time>` value).
fn parse_single_animation(text: &str) -> Option<SingleAnimation> {
    if text.is_empty() {
        return None;
    }

    let mut anim = SingleAnimation::default();
    for token in split_top_level(text, b' ') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if is_time(token) {
            if anim.duration.is_none() {
                anim.duration = Some(token.to_string());
            } else if anim.delay.is_none() {
                anim.delay = Some(token.to_string());
            } else {
                return None;
            }
        } else if is_timing_function(token) {
            assign_once(&mut anim.timing_function, token)?;
        } else if is_iteration_count(token) {
            assign_once(&mut anim.iteration_count, token)?;
        } else if is_direction(token) {
            assign_once(&mut anim.direction, token)?;
        } else if is_play_state(token) {
            assign_once(&mut anim.play_state, token)?;
        } else if token.eq_ignore_ascii_case("none") {
            // `none` is valid for both animation-name and animation-fill-mode.
            // Browsers assign it to the name slot first, then fall back to
            // fill-mode once a name has already been seen.
            if !anim.name_seen {
                anim.name_seen = true;
            } else if anim.fill_mode.is_none() {
                anim.fill_mode = Some("none".to_string());
            } else {
                return None;
            }
        } else if is_fill_mode(token) {
            assign_once(&mut anim.fill_mode, token)?;
        } else if is_name(token) {
            if anim.name_seen {
                return None;
            }
            anim.name_seen = true;
            anim.name = Some(token.to_string());
        } else {
            return None;
        }
    }

    Some(anim)
}

fn assign_once(slot: &mut Option<String>, token: &str) -> Option<()> {
    if slot.is_some() {
        return None;
    }
    *slot = Some(token.to_string());
    Some(())
}

fn is_time(token: &str) -> bool {
    let number = match strip_suffix_ci(token, "ms").or_else(|| strip_suffix_ci(token, "s")) {
        Some(number) => number,
        None => return false,
    };
    !number.is_empty() && number.parse::<f64>().map(f64::is_finite).unwrap_or(false)
}

fn is_timing_function(token: &str) -> bool {
    const KEYWORDS: [&str; 7] = [
        "linear",
        "ease",
        "ease-in",
        "ease-out",
        "ease-in-out",
        "step-start",
        "step-end",
    ];
    if KEYWORDS.iter().any(|k| token.eq_ignore_ascii_case(k)) {
        return true;
    }
    token.ends_with(')')
        && (starts_with_ci(token, "steps(") || starts_with_ci(token, "cubic-bezier("))
}

fn is_iteration_count(token: &str) -> bool {
    token.eq_ignore_ascii_case("infinite")
        || token
            .parse::<f64>()
            .map(|n| n.is_finite() && n >= 0.0)
            .unwrap_or(false)
}

fn is_direction(token: &str) -> bool {
    ["normal", "reverse", "alternate", "alternate-reverse"]
        .iter()
        .any(|k| token.eq_ignore_ascii_case(k))
}

fn is_play_state(token: &str) -> bool {
    token.eq_ignore_ascii_case("running") || token.eq_ignore_ascii_case("paused")
}

fn is_fill_mode(token: &str) -> bool {
    // `none` is handled by the caller because it is ambiguous with a name.
    ["forwards", "backwards", "both"]
        .iter()
        .any(|k| token.eq_ignore_ascii_case(k))
}

fn is_name(token: &str) -> bool {
    if [
        "initial", "inherit", "unset", "revert", "default", "none",
    ]
    .iter()
    .any(|k| token.eq_ignore_ascii_case(k))
    {
        return false;
    }

    match token.chars().next() {
        Some('"') | Some('\'') => true,
        Some(c) if c == '-' || c == '_' || c.is_ascii_alphabetic() || !c.is_ascii() => true,
        _ => false,
    }
}

fn strip_suffix_ci<'a>(token: &'a str, suffix: &str) -> Option<&'a str> {
    if token.len() < suffix.len() {
        return None;
    }
    let split = token.len() - suffix.len();
    let (head, tail) = token.split_at(split);
    tail.eq_ignore_ascii_case(suffix).then_some(head)
}

fn starts_with_ci(token: &str, prefix: &str) -> bool {
    token.len() >= prefix.len() && token[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Splits `text` on `delimiter` at the top level, keeping delimiters that appear
/// inside parentheses or quoted strings together with their surroundings.
fn split_top_level(text: &str, delimiter: u8) -> Vec<&str> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'"' | b'\'' => {
                i = skip_string(bytes, i, bytes[i]);
                continue;
            }
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b if b == delimiter && depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    parts.push(&text[start..len]);
    parts
}

fn skip_string(bytes: &[u8], i: usize, quote: u8) -> usize {
    let len = bytes.len();
    let mut j = i + 1;
    while j < len {
        let c = bytes[j];
        if c == b'\\' {
            j += 2;
        } else if c == quote {
            return j + 1;
        } else {
            j += 1;
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(value: &str) -> AnimationLonghands {
        expand_animation_shorthand(value).expect("valid shorthand")
    }

    #[test]
    fn full_single_animation() {
        let longhands = expand("move 4s steps(4, jump-end) both");
        assert_eq!(longhands.name, "move");
        assert_eq!(longhands.duration, "4s");
        assert_eq!(longhands.timing_function, "steps(4, jump-end)");
        assert_eq!(longhands.fill_mode, "both");
        assert_eq!(longhands.delay, "0s");
        assert_eq!(longhands.iteration_count, "1");
        assert_eq!(longhands.direction, "normal");
        assert_eq!(longhands.play_state, "running");
    }

    #[test]
    fn duration_then_delay() {
        let longhands = expand("move 4s 3s");
        assert_eq!(longhands.duration, "4s");
        assert_eq!(longhands.delay, "3s");
    }

    #[test]
    fn iteration_count_and_direction_and_play_state() {
        let longhands = expand("spin 1s infinite alternate paused");
        assert_eq!(longhands.iteration_count, "infinite");
        assert_eq!(longhands.direction, "alternate");
        assert_eq!(longhands.play_state, "paused");
    }

    #[test]
    fn duration_only_has_no_name() {
        let longhands = expand("4s");
        assert_eq!(longhands.name, "none");
        assert_eq!(longhands.duration, "4s");
    }

    #[test]
    fn multi_animation_lists_are_index_matched() {
        let longhands = expand("spin 1s linear infinite, fade 2s ease-out");
        assert_eq!(longhands.name, "spin, fade");
        assert_eq!(longhands.duration, "1s, 2s");
        assert_eq!(longhands.timing_function, "linear, ease-out");
        assert_eq!(longhands.iteration_count, "infinite, 1");
        assert_eq!(longhands.delay, "0s, 0s");
    }

    #[test]
    fn three_time_values_is_invalid() {
        assert!(expand_animation_shorthand("4s 3s 2s move").is_none());
    }

    #[test]
    fn duplicate_category_is_invalid() {
        assert!(expand_animation_shorthand("move linear ease").is_none());
        assert!(expand_animation_shorthand("a b").is_none());
    }

    #[test]
    fn steps_keeps_inner_comma_and_space() {
        let longhands = expand("4s steps(4, jump-end)");
        assert_eq!(longhands.timing_function, "steps(4, jump-end)");
        assert_eq!(longhands.duration, "4s");
    }

    #[test]
    fn none_becomes_name_then_fill_mode() {
        let single = expand("none");
        assert_eq!(single.name, "none");
        assert_eq!(single.fill_mode, "none");

        let with_name = expand("move none");
        assert_eq!(with_name.name, "move");
        assert_eq!(with_name.fill_mode, "none");
    }

    #[test]
    fn empty_animation_is_invalid() {
        assert!(expand_animation_shorthand("a,").is_none());
    }
}
