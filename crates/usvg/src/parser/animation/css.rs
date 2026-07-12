// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CSS `@keyframes` extraction and conversion to the typed animation model.
//!
//! `simplecss` only understands selector-based rules, so `@keyframes` blocks are
//! pulled out here before the remaining text is handed to it. The parsed rules
//! are later matched against each element's `animation-*` properties and
//! converted into typed animations.

use std::str::FromStr;
use std::sync::Arc;

use crate::parser::svgtree::{AId, Document, EId, SvgNode};
use crate::tree::animation::{
    Accumulate, Additive, Animation, AnimationKind, AnimationSource, CalcMode, CssFillMode,
    CssTiming, Direction, Easing, Iterations, Keyframe, PlayState, StepPosition, Timing,
    TimingFunction, Track, TransformBox, TransformFunction, TransformOrigin, TransformOriginValue,
    TransformTrack,
};
use crate::{NormalizedF32, Opacity};

/// A parsed `@keyframes` rule.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeyframesRule {
    /// The animation name used by `animation`/`animation-name`.
    pub(crate) name: String,
    /// The keyframes in source order.
    pub(crate) keyframes: Vec<CssKeyframe>,
}

/// A single keyframe inside a `@keyframes` rule.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CssKeyframe {
    /// Selector offsets in the `0.0..=1.0` range (`from` is `0.0`, `to` is `1.0`, `N%` is `N/100`).
    pub(crate) offsets: Vec<f32>,
    /// Property/value declarations, excluding `animation-timing-function`.
    pub(crate) declarations: Vec<(String, String)>,
    /// The per-keyframe `animation-timing-function`, if present.
    pub(crate) timing_function: Option<String>,
}

/// Extracts every `@keyframes` block from `css`.
///
/// Returns the parsed rules and the remaining CSS with the `@keyframes` blocks
/// removed. Duplicate rule names keep the last definition.
pub(crate) fn extract_keyframes(css: &str) -> (Vec<KeyframesRule>, String) {
    let bytes = css.as_bytes();
    let len = bytes.len();
    let mut rules: Vec<KeyframesRule> = Vec::new();
    let mut remaining = String::with_capacity(len);
    let mut copy_from = 0;
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i = skip_comment(bytes, i);
            }
            b'"' | b'\'' => {
                i = skip_string(bytes, i, bytes[i]);
            }
            b'@' if matches_keyword(bytes, i + 1, b"keyframes") => {
                remaining.push_str(&css[copy_from..i]);
                match parse_keyframes_block(css, i) {
                    Some((rule, end)) => {
                        insert_last_wins(&mut rules, rule);
                        i = end;
                        copy_from = end;
                    }
                    None => {
                        log::warn!("Unterminated @keyframes block; skipped.");
                        copy_from = len;
                        i = len;
                    }
                }
            }
            b'@' if matches_keyword(bytes, i + 1, b"import") => {
                remaining.push_str(&css[copy_from..i]);
                log::warn!("External CSS is not supported.");
                i = skip_at_statement(bytes, i);
                copy_from = i;
            }
            _ => {
                i += 1;
            }
        }
    }

    if copy_from < len {
        remaining.push_str(&css[copy_from..len]);
    }

    (rules, remaining)
}

fn insert_last_wins(rules: &mut Vec<KeyframesRule>, rule: KeyframesRule) {
    if let Some(existing) = rules.iter_mut().find(|r| r.name == rule.name) {
        *existing = rule;
    } else {
        rules.push(rule);
    }
}

/// Parses a `@keyframes` rule starting at the `@` byte.
///
/// Returns the rule and the index just past its closing brace, or `None` when
/// the block never closes.
fn parse_keyframes_block(css: &str, at: usize) -> Option<(KeyframesRule, usize)> {
    let bytes = css.as_bytes();
    let len = bytes.len();

    let mut i = skip_ws_comments(bytes, at + 1 + "keyframes".len());
    let (name, after_name) = read_name(css, i);
    i = skip_ws_comments(bytes, after_name);
    if i >= len || bytes[i] != b'{' {
        return None;
    }

    let body_start = i + 1;
    let body_end = find_block_end(bytes, i)?;
    let keyframes = parse_block_body(&css[body_start..body_end]);
    Some((KeyframesRule { name, keyframes }, body_end + 1))
}

fn read_name(css: &str, i: usize) -> (String, usize) {
    let bytes = css.as_bytes();
    let len = bytes.len();
    if i < len && (bytes[i] == b'"' || bytes[i] == b'\'') {
        let quote = bytes[i];
        let end = skip_string(bytes, i, quote);
        let inner_end = if end > i + 1 && bytes[end - 1] == quote {
            end - 1
        } else {
            end
        };
        (css.get(i + 1..inner_end).unwrap_or("").to_string(), end)
    } else {
        let mut j = i;
        while j < len {
            let c = bytes[j];
            if c.is_ascii_whitespace()
                || c == b'{'
                || (c == b'/' && j + 1 < len && bytes[j + 1] == b'*')
            {
                break;
            }
            j += 1;
        }
        (css[i..j].trim().to_string(), j)
    }
}

fn parse_block_body(body: &str) -> Vec<CssKeyframe> {
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut keyframes = Vec::new();
    let mut i = 0;

    loop {
        i = skip_ws_comments(bytes, i);
        if i >= len {
            break;
        }

        let selector_start = i;
        while i < len {
            match bytes[i] {
                b'/' if i + 1 < len && bytes[i + 1] == b'*' => i = skip_comment(bytes, i),
                b'{' => break,
                _ => i += 1,
            }
        }
        if i >= len {
            break;
        }

        let selector = &body[selector_start..i];
        let decl_start = i + 1;
        let (decl_end, next) = match find_block_end(bytes, i) {
            Some(end) => (end, end + 1),
            None => (len, len),
        };
        i = next;

        let offsets = parse_selectors(selector);
        if offsets.is_empty() {
            continue;
        }
        let (declarations, timing_function) = parse_declarations(&body[decl_start..decl_end]);
        keyframes.push(CssKeyframe {
            offsets,
            declarations,
            timing_function,
        });
    }

    keyframes
}

fn parse_selectors(selector: &str) -> Vec<f32> {
    let mut offsets = Vec::new();
    for part in selector.split(',') {
        let cleaned = strip_comments(part);
        let token = cleaned.trim();
        if token.eq_ignore_ascii_case("from") {
            offsets.push(0.0);
        } else if token.eq_ignore_ascii_case("to") {
            offsets.push(1.0);
        } else if let Some(percent) = token.strip_suffix('%') {
            if let Ok(value) = percent.trim().parse::<f32>() {
                if value.is_finite() {
                    offsets.push(value / 100.0);
                }
            }
        }
    }
    offsets
}

fn parse_declarations(text: &str) -> (Vec<(String, String)>, Option<String>) {
    let mut declarations = Vec::new();
    let mut timing_function = None;

    for piece in split_top_level(text, b';') {
        let Some((property_raw, value_raw)) = split_property(piece) else {
            continue;
        };
        let property_owned = strip_comments(property_raw);
        let property = property_owned.trim();
        let value_owned = strip_comments(value_raw);
        let value = value_owned.trim();
        if property.is_empty() || value.is_empty() {
            continue;
        }
        if property.eq_ignore_ascii_case("animation-timing-function") {
            timing_function = Some(value.to_string());
        } else {
            declarations.push((property.to_string(), value.to_string()));
        }
    }

    (declarations, timing_function)
}

fn split_property(piece: &str) -> Option<(&str, &str)> {
    let bytes = piece.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut paren_depth = 0usize;
    while i < len {
        match bytes[i] {
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i = skip_comment(bytes, i);
                continue;
            }
            b'"' | b'\'' => {
                i = skip_string(bytes, i, bytes[i]);
                continue;
            }
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b':' if paren_depth == 0 => return Some((&piece[..i], &piece[i + 1..])),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Splits `text` on `delimiter` at the top level, ignoring delimiters inside
/// strings, parentheses and comments.
fn split_top_level(text: &str, delimiter: u8) -> Vec<&str> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut paren_depth = 0usize;
    while i < len {
        match bytes[i] {
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i = skip_comment(bytes, i);
                continue;
            }
            b'"' | b'\'' => {
                i = skip_string(bytes, i, bytes[i]);
                continue;
            }
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b if b == delimiter && paren_depth == 0 => {
                pieces.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    pieces.push(&text[start..len]);
    pieces
}

fn strip_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut copy_from = 0;
    let mut i = 0;
    while i < len {
        if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            out.push_str(&text[copy_from..i]);
            i = skip_comment(bytes, i);
            copy_from = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[copy_from..len]);
    out
}

/// Finds the index of the `}` matching the `{` at `open`, tracking nested braces
/// while ignoring braces inside strings and comments.
fn find_block_end(bytes: &[u8], open: usize) -> Option<usize> {
    let len = bytes.len();
    let mut depth = 0usize;
    let mut i = open;
    while i < len {
        match bytes[i] {
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i = skip_comment(bytes, i);
                continue;
            }
            b'"' | b'\'' => {
                i = skip_string(bytes, i, bytes[i]);
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Skips an at-statement (such as `@import`) up to and including its terminating
/// `;`, or past its block when one is present.
fn skip_at_statement(bytes: &[u8], at: usize) -> usize {
    let len = bytes.len();
    let mut i = at + 1;
    while i < len {
        match bytes[i] {
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i = skip_comment(bytes, i);
                continue;
            }
            b'"' | b'\'' => {
                i = skip_string(bytes, i, bytes[i]);
                continue;
            }
            b';' => return i + 1,
            b'{' => return find_block_end(bytes, i).map_or(len, |end| end + 1),
            _ => i += 1,
        }
    }
    len
}

fn skip_ws_comments(bytes: &[u8], mut i: usize) -> usize {
    let len = bytes.len();
    loop {
        if i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        } else if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i = skip_comment(bytes, i);
        } else {
            return i;
        }
    }
}

/// Skips a `/* ... */` comment starting at `i`, returning the index past it.
fn skip_comment(bytes: &[u8], i: usize) -> usize {
    let len = bytes.len();
    let mut j = i + 2;
    while j < len {
        if bytes[j] == b'*' && j + 1 < len && bytes[j + 1] == b'/' {
            return j + 2;
        }
        j += 1;
    }
    len
}

/// Skips a `"..."` or `'...'` string starting at `i`, returning the index past
/// the closing quote and honouring backslash escapes.
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

fn matches_keyword(bytes: &[u8], start: usize, keyword: &[u8]) -> bool {
    let end = start + keyword.len();
    if end > bytes.len() {
        return false;
    }
    for (offset, &expected) in keyword.iter().enumerate() {
        if !bytes[start + offset].eq_ignore_ascii_case(&expected) {
            return false;
        }
    }
    match bytes.get(end) {
        Some(&b) => !is_ident_byte(b),
        None => true,
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Builds the CSS animations attached to `node` via its `animation-name`.
///
/// Each `animation-name` is matched against the document's `@keyframes` rules
/// and expanded into one [`Animation`] per animated CSS property. Unsupported
/// properties, unknown keyframes names and CSS variables are dropped with a
/// warning.
///
/// A rule that omits the `0%`/`100%` keyframes keeps only its explicit
/// keyframes; the sampler supplies the underlying value at the missing edges.
pub(crate) fn build_css_animations<'a, 'input>(
    node: SvgNode<'a, 'input>,
    doc: &'a Document<'input>,
) -> Vec<Arc<Animation>> {
    let Some(names) = node.attribute::<&str>(AId::AnimationName) else {
        return Vec::new();
    };

    let names = split_list(names);
    let durations = longhand_list(node, AId::AnimationDuration);
    let delays = longhand_list(node, AId::AnimationDelay);
    let iteration_counts = longhand_list(node, AId::AnimationIterationCount);
    let directions = longhand_list(node, AId::AnimationDirection);
    let fill_modes = longhand_list(node, AId::AnimationFillMode);
    let timing_functions = longhand_list(node, AId::AnimationTimingFunction);
    let play_states = longhand_list(node, AId::AnimationPlayState);

    let is_stop = node.tag_name() == Some(EId::Stop);
    let origin = read_transform_origin(node);
    let box_ = read_transform_box(node);

    let mut animations = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let name = name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            continue;
        }

        let Some(rule) = doc.keyframes().iter().find(|rule| rule.name.as_str() == name) else {
            log::warn!("Unknown keyframes name: '{}'.", name);
            continue;
        };

        let timing = CssTiming::new(
            parse_time(cycle(&durations, index).unwrap_or("0s")).unwrap_or(0.0),
            parse_time(cycle(&delays, index).unwrap_or("0s")).unwrap_or(0.0),
            parse_iterations(cycle(&iteration_counts, index).unwrap_or("1")),
            parse_direction(cycle(&directions, index).unwrap_or("normal")),
            parse_fill_mode(cycle(&fill_modes, index).unwrap_or("none")),
            parse_timing_function(cycle(&timing_functions, index).unwrap_or("ease"))
                .unwrap_or(TimingFunction::Linear),
            parse_play_state(cycle(&play_states, index).unwrap_or("running")),
        );

        for property in animated_properties(rule) {
            if let Some(animation) =
                build_property_animation(node, rule, &property, is_stop, timing, origin, box_)
            {
                animations.push(animation);
            }
        }
    }

    animations
}

/// The CSS properties whose `@keyframes` values this crate converts.
enum CssProperty {
    Transform,
    Opacity,
    Fill,
    Stroke,
    StrokeWidth,
    StrokeDashoffset,
    StopColor,
    StopOpacity,
}

/// Builds a single property animation from one `@keyframes` rule.
fn build_property_animation(
    node: SvgNode,
    rule: &KeyframesRule,
    property: &str,
    is_stop: bool,
    timing: CssTiming,
    origin: TransformOrigin,
    box_: TransformBox,
) -> Option<Arc<Animation>> {
    let property = property.trim();
    if property.starts_with("--") {
        log::warn!("CSS variables are not supported.");
        return None;
    }

    let Some(css_property) = classify_property(property, is_stop) else {
        log::warn!("Unsupported CSS property in keyframes: '{}'.", property);
        return None;
    };

    let entries = property_entries(rule, property);
    if entries.iter().any(|(_, value, _)| value.contains("var(")) {
        log::warn!("CSS variables are not supported.");
        return None;
    }

    let kind = match css_property {
        CssProperty::Transform => {
            let keyframes = typed_keyframes(&entries, parse_transform_functions);
            if keyframes.is_empty() {
                return None;
            }
            AnimationKind::Transform(TransformTrack::Css {
                keyframes,
                origin,
                box_,
            })
        }
        CssProperty::Opacity => AnimationKind::Opacity(build_track(&entries, parse_css_opacity)?),
        CssProperty::Fill => AnimationKind::Fill(build_track(&entries, parse_css_color)?),
        CssProperty::Stroke => AnimationKind::Stroke(build_track(&entries, parse_css_color)?),
        CssProperty::StrokeWidth => {
            AnimationKind::StrokeWidth(build_track(&entries, parse_css_number)?)
        }
        CssProperty::StrokeDashoffset => {
            AnimationKind::StrokeDashoffset(build_track(&entries, parse_css_number)?)
        }
        CssProperty::StopColor => AnimationKind::StopColor(build_track(&entries, parse_css_color)?),
        CssProperty::StopOpacity => {
            AnimationKind::StopOpacity(build_track(&entries, parse_css_opacity)?)
        }
    };

    Some(Arc::new(Animation::new(
        kind,
        Timing::Css(timing),
        Easing::new(CalcMode::Linear, None, None),
        Additive::Replace,
        Accumulate::None,
        AnimationSource::Css,
        property_suppressed_by_important(node, property),
    )))
}

/// Classifies a CSS property name against the supported set.
///
/// `stop-color`/`stop-opacity` are only admitted on `<stop>` targets.
fn classify_property(property: &str, is_stop: bool) -> Option<CssProperty> {
    if property.eq_ignore_ascii_case("transform") {
        Some(CssProperty::Transform)
    } else if property.eq_ignore_ascii_case("opacity") {
        Some(CssProperty::Opacity)
    } else if property.eq_ignore_ascii_case("fill") {
        Some(CssProperty::Fill)
    } else if property.eq_ignore_ascii_case("stroke") {
        Some(CssProperty::Stroke)
    } else if property.eq_ignore_ascii_case("stroke-width") {
        Some(CssProperty::StrokeWidth)
    } else if property.eq_ignore_ascii_case("stroke-dashoffset") {
        Some(CssProperty::StrokeDashoffset)
    } else if is_stop && property.eq_ignore_ascii_case("stop-color") {
        Some(CssProperty::StopColor)
    } else if is_stop && property.eq_ignore_ascii_case("stop-opacity") {
        Some(CssProperty::StopOpacity)
    } else {
        None
    }
}

/// Collects the distinct property names animated by a rule, in first-seen order.
fn animated_properties(rule: &KeyframesRule) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for keyframe in &rule.keyframes {
        for (property, _) in &keyframe.declarations {
            let property = property.trim();
            if !names.iter().any(|existing| existing.eq_ignore_ascii_case(property)) {
                names.push(property.to_string());
            }
        }
    }
    names
}

/// Gathers a single property's `(offset, value, timing-function)` entries,
/// sorted by offset.
fn property_entries<'r>(
    rule: &'r KeyframesRule,
    property: &str,
) -> Vec<(f32, &'r str, Option<&'r str>)> {
    let mut entries = Vec::new();
    for keyframe in &rule.keyframes {
        let Some((_, value)) = keyframe
            .declarations
            .iter()
            .find(|(name, _)| name.trim().eq_ignore_ascii_case(property))
        else {
            continue;
        };
        for &offset in &keyframe.offsets {
            entries.push((offset, value.as_str(), keyframe.timing_function.as_deref()));
        }
    }
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));
    entries
}

/// Builds a typed track, returning `None` when no keyframe value parses.
fn build_track<T: Clone>(
    entries: &[(f32, &str, Option<&str>)],
    parse: impl Fn(&str) -> Option<T>,
) -> Option<Track<T>> {
    let keyframes = typed_keyframes(entries, parse);
    (!keyframes.is_empty()).then(|| Track::new(keyframes))
}

/// Parses each entry's value into a typed keyframe, dropping unparsable ones.
fn typed_keyframes<T: Clone>(
    entries: &[(f32, &str, Option<&str>)],
    parse: impl Fn(&str) -> Option<T>,
) -> Vec<Keyframe<T>> {
    entries
        .iter()
        .copied()
        .filter_map(|(offset, value, timing)| {
            Some(Keyframe::new(
                NormalizedF32::new_clamped(offset),
                parse(value.trim())?,
                timing.and_then(parse_timing_function),
            ))
        })
        .collect()
}

/// Returns whether a winning `!important` static declaration suppresses the
/// property's animation.
fn property_suppressed_by_important(node: SvgNode, property: &str) -> bool {
    AId::from_str(property).is_some_and(|aid| {
        node.attributes()
            .iter()
            .find(|item| item.name == aid)
            .is_some_and(|item| item.important)
    })
}

fn read_transform_origin(node: SvgNode) -> TransformOrigin {
    match node.try_attribute::<svgtypes::TransformOrigin>(AId::TransformOrigin) {
        Some(origin) => {
            TransformOrigin::new(origin_component(origin.x_offset), origin_component(origin.y_offset))
        }
        None => TransformOrigin::new(
            TransformOriginValue::Percent(50.0),
            TransformOriginValue::Percent(50.0),
        ),
    }
}

fn origin_component(length: svgtypes::Length) -> TransformOriginValue {
    if length.unit == svgtypes::LengthUnit::Percent {
        TransformOriginValue::Percent(length.number as f32)
    } else {
        TransformOriginValue::Length(length.number as f32)
    }
}

fn read_transform_box(node: SvgNode) -> TransformBox {
    match node.try_attribute::<&str>(AId::TransformBox).map(str::trim) {
        Some("content-box") => TransformBox::ContentBox,
        Some("border-box") => TransformBox::BorderBox,
        Some("fill-box") => TransformBox::FillBox,
        Some("stroke-box") => TransformBox::StrokeBox,
        _ => TransformBox::ViewBox,
    }
}

fn split_list(value: &str) -> Vec<&str> {
    // A single value such as `steps(4, jump-end)` may carry its own commas, so
    // the list is split at the top level only.
    split_top_level(value, b',').into_iter().map(str::trim).collect()
}

fn longhand_list<'a>(node: SvgNode<'a, '_>, aid: AId) -> Vec<&'a str> {
    node.attribute::<&str>(aid).map(split_list).unwrap_or_default()
}

/// Reads the `index`th list entry, cycling as CSS does when a longhand list is
/// shorter than the `animation-name` list.
fn cycle<'a>(list: &[&'a str], index: usize) -> Option<&'a str> {
    if list.is_empty() {
        None
    } else {
        Some(list[index % list.len()])
    }
}

fn parse_time(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(number) = strip_suffix_ci(value, "ms") {
        return parse_finite(number).map(|seconds| seconds / 1000.0);
    }
    if let Some(number) = strip_suffix_ci(value, "s") {
        return parse_finite(number);
    }
    parse_finite(value)
}

fn parse_iterations(value: &str) -> Iterations {
    if value.trim().eq_ignore_ascii_case("infinite") {
        return Iterations::Infinite;
    }
    match parse_finite(value) {
        Some(count) if count >= 0.0 => Iterations::Count(count),
        _ => Iterations::Count(1.0),
    }
}

fn parse_direction(value: &str) -> Direction {
    let value = value.trim();
    if value.eq_ignore_ascii_case("reverse") {
        Direction::Reverse
    } else if value.eq_ignore_ascii_case("alternate") {
        Direction::Alternate
    } else if value.eq_ignore_ascii_case("alternate-reverse") {
        Direction::AlternateReverse
    } else {
        Direction::Normal
    }
}

fn parse_fill_mode(value: &str) -> CssFillMode {
    let value = value.trim();
    if value.eq_ignore_ascii_case("forwards") {
        CssFillMode::Forwards
    } else if value.eq_ignore_ascii_case("backwards") {
        CssFillMode::Backwards
    } else if value.eq_ignore_ascii_case("both") {
        CssFillMode::Both
    } else {
        CssFillMode::None
    }
}

fn parse_play_state(value: &str) -> PlayState {
    if value.trim().eq_ignore_ascii_case("paused") {
        PlayState::Paused
    } else {
        PlayState::Running
    }
}

fn parse_timing_function(value: &str) -> Option<TimingFunction> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("linear") {
        return Some(TimingFunction::Linear);
    }
    if value.eq_ignore_ascii_case("ease") {
        return Some(TimingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0));
    }
    if value.eq_ignore_ascii_case("ease-in") {
        return Some(TimingFunction::CubicBezier(0.42, 0.0, 1.0, 1.0));
    }
    if value.eq_ignore_ascii_case("ease-out") {
        return Some(TimingFunction::CubicBezier(0.0, 0.0, 0.58, 1.0));
    }
    if value.eq_ignore_ascii_case("ease-in-out") {
        return Some(TimingFunction::CubicBezier(0.42, 0.0, 0.58, 1.0));
    }
    if value.eq_ignore_ascii_case("step-start") {
        return Some(TimingFunction::Steps(1, StepPosition::JumpStart));
    }
    if value.eq_ignore_ascii_case("step-end") {
        return Some(TimingFunction::Steps(1, StepPosition::JumpEnd));
    }
    if let Some(arguments) = function_arguments(value, "steps") {
        return parse_steps(arguments);
    }
    if let Some(arguments) = function_arguments(value, "cubic-bezier") {
        return parse_cubic_bezier(arguments);
    }
    None
}

/// Returns the argument list of a `name(...)` functional value.
fn function_arguments<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let inner = value.strip_suffix(')')?;
    let open = inner.find('(')?;
    inner[..open]
        .trim()
        .eq_ignore_ascii_case(name)
        .then(|| &inner[open + 1..])
}

fn parse_steps(arguments: &str) -> Option<TimingFunction> {
    let mut parts = arguments.split(',');
    let count: u32 = parts.next()?.trim().parse().ok()?;
    if count == 0 {
        return None;
    }
    let position = match parts.next() {
        Some(keyword) => parse_step_position(keyword.trim())?,
        None => StepPosition::JumpEnd,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(TimingFunction::Steps(count, position))
}

fn parse_step_position(keyword: &str) -> Option<StepPosition> {
    match keyword {
        "jump-start" | "start" => Some(StepPosition::JumpStart),
        "jump-end" | "end" => Some(StepPosition::JumpEnd),
        "jump-none" => Some(StepPosition::JumpNone),
        "jump-both" => Some(StepPosition::JumpBoth),
        _ => None,
    }
}

fn parse_cubic_bezier(arguments: &str) -> Option<TimingFunction> {
    let mut values = [0.0f32; 4];
    let mut count = 0;
    for part in arguments.split(',') {
        *values.get_mut(count)? = parse_finite(part.trim())?;
        count += 1;
    }
    (count == 4).then(|| TimingFunction::CubicBezier(values[0], values[1], values[2], values[3]))
}

fn parse_css_opacity(value: &str) -> Option<Opacity> {
    let length = svgtypes::Length::from_str(value).ok()?;
    match length.unit {
        svgtypes::LengthUnit::Percent => Some(Opacity::new_clamped(length.number as f32 / 100.0)),
        svgtypes::LengthUnit::None => Some(Opacity::new_clamped(length.number as f32)),
        _ => None,
    }
}

fn parse_css_color(value: &str) -> Option<svgtypes::Color> {
    svgtypes::Color::from_str(value).ok()
}

fn parse_css_number(value: &str) -> Option<f32> {
    let length = svgtypes::Length::from_str(value).ok()?;
    length.number.is_finite().then_some(length.number as f32)
}

/// Parses a CSS `transform` value into a list of transform functions.
fn parse_transform_functions(value: &str) -> Option<Vec<TransformFunction>> {
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
        TransformFunction::Matrix(values[0], values[1], values[2], values[3], values[4], values[5])
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

fn parse_finite(value: &str) -> Option<f32> {
    let number: f32 = value.trim().parse().ok()?;
    number.is_finite().then_some(number)
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

fn strip_suffix_ci<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
    if value.len() < suffix.len() {
        return None;
    }
    let split = value.len() - suffix.len();
    let (head, tail) = value.split_at(split);
    tail.eq_ignore_ascii_case(suffix).then_some(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_braces_are_balanced() {
        let (rules, remaining) =
            extract_keyframes("@keyframes spin { 0% { fill: red } } .foo { fill: blue }");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "spin");
        assert_eq!(rules[0].keyframes.len(), 1);
        assert_eq!(rules[0].keyframes[0].offsets, vec![0.0]);
        assert_eq!(
            rules[0].keyframes[0].declarations,
            vec![("fill".to_string(), "red".to_string())]
        );
        assert!(remaining.contains(".foo { fill: blue }"));
        assert!(!remaining.contains("@keyframes"));
    }

    #[test]
    fn closing_brace_inside_string_does_not_close_block() {
        let (rules, remaining) = extract_keyframes(
            "@keyframes a { 0% { content: \"}\"; fill: red } 100% { fill: blue } } rect { x: 1 }",
        );
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].keyframes.len(), 2);
        assert_eq!(
            rules[0].keyframes[0].declarations,
            vec![
                ("content".to_string(), "\"}\"".to_string()),
                ("fill".to_string(), "red".to_string()),
            ]
        );
        assert_eq!(rules[0].keyframes[1].offsets, vec![1.0]);
        assert!(remaining.contains("rect { x: 1 }"));
        assert!(!remaining.contains("@keyframes"));
    }

    #[test]
    fn keyframes_inside_comment_is_not_extracted() {
        let input = "/* @keyframes fake { 0% { fill: red } } */ a { fill: green }";
        let (rules, remaining) = extract_keyframes(input);
        assert!(rules.is_empty());
        assert_eq!(remaining, input);
    }

    #[test]
    fn multiple_blocks_are_extracted() {
        let (rules, remaining) = extract_keyframes(
            "@keyframes a { from { opacity: 0 } } @keyframes b { to { opacity: 1 } } rect { x: 5 }",
        );
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].name, "a");
        assert_eq!(rules[0].keyframes[0].offsets, vec![0.0]);
        assert_eq!(rules[1].name, "b");
        assert_eq!(rules[1].keyframes[0].offsets, vec![1.0]);
        assert!(remaining.contains("rect { x: 5 }"));
        assert!(!remaining.contains("@keyframes"));
    }

    #[test]
    fn duplicate_names_keep_last() {
        let (rules, _) = extract_keyframes(
            "@keyframes a { from { opacity: 0 } } @keyframes a { from { opacity: 1 } }",
        );
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "a");
        assert_eq!(
            rules[0].keyframes[0].declarations,
            vec![("opacity".to_string(), "1".to_string())]
        );
    }

    #[test]
    fn unterminated_block_is_skipped() {
        let (rules, remaining) =
            extract_keyframes("rect { x: 1 } @keyframes a { from { opacity: 0 }");
        assert!(rules.is_empty());
        assert!(remaining.contains("rect { x: 1 }"));
        assert!(!remaining.contains("@keyframes"));
    }

    #[test]
    fn import_is_dropped() {
        let (rules, remaining) =
            extract_keyframes("@import url(\"theme.css\"); a { fill: red }");
        assert!(rules.is_empty());
        assert!(!remaining.contains("@import"));
        assert!(remaining.contains("a { fill: red }"));
    }

    #[test]
    fn selector_forms_and_comma_lists() {
        let (rules, _) = extract_keyframes("@keyframes a { 0%, 100% { opacity: 1 } 50% { opacity: 0.5 } }");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].keyframes[0].offsets, vec![0.0, 1.0]);
        assert_eq!(rules[0].keyframes[1].offsets, vec![0.5]);
    }

    #[test]
    fn timing_function_is_split_out() {
        let (rules, _) = extract_keyframes(
            "@keyframes a { from { opacity: 0; animation-timing-function: ease-in } to { opacity: 1 } }",
        );
        let first = &rules[0].keyframes[0];
        assert_eq!(first.timing_function.as_deref(), Some("ease-in"));
        assert_eq!(
            first.declarations,
            vec![("opacity".to_string(), "0".to_string())]
        );
        assert_eq!(rules[0].keyframes[1].timing_function, None);
    }

    #[test]
    fn declaration_split_respects_parens_and_strings() {
        let (rules, _) = extract_keyframes(
            "@keyframes a { 50% { transform: translate(1px, 2px); content: \";\" } }",
        );
        assert_eq!(
            rules[0].keyframes[0].declarations,
            vec![
                ("transform".to_string(), "translate(1px, 2px)".to_string()),
                ("content".to_string(), "\";\"".to_string()),
            ]
        );
    }

    #[test]
    fn empty_input_yields_no_rules() {
        let (rules, remaining) = extract_keyframes("");
        assert!(rules.is_empty());
        assert_eq!(remaining, "");
    }

    #[test]
    fn plain_css_passes_through_unchanged() {
        let input = "rect { fill: red } .a { stroke: blue }";
        let (rules, remaining) = extract_keyframes(input);
        assert!(rules.is_empty());
        assert_eq!(remaining, input);
    }
}
