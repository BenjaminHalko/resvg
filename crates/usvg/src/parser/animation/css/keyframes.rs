// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::declarations::parse_block_body;
use super::scanner::{
    find_block_end, matches_keyword, skip_at_statement, skip_comment, skip_string, skip_ws_comments,
};

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

    let i = skip_ws_comments(bytes, at + 1 + "keyframes".len());
    let (name, i) = read_name(css, i);
    let i = skip_ws_comments(bytes, i);
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
        (css[i + 1..inner_end].to_string(), end)
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
        let (rules, remaining) = extract_keyframes("@import url(\"theme.css\"); a { fill: red }");
        assert!(rules.is_empty());
        assert!(!remaining.contains("@import"));
        assert!(remaining.contains("a { fill: red }"));
    }

    #[test]
    fn selector_forms_and_comma_lists() {
        let (rules, _) =
            extract_keyframes("@keyframes a { 0%, 100% { opacity: 1 } 50% { opacity: 0.5 } }");
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
