// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Extraction of `@keyframes` rules from CSS text.
//!
//! `simplecss` only understands selector-based rules, so `@keyframes` blocks are
//! pulled out here before the remaining text is handed to it.

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
