// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::keyframes::CssKeyframe;
use super::scanner::{
    find_block_end, skip_comment, skip_string, skip_ws_comments, split_top_level,
};

pub(super) fn parse_block_body(body: &str) -> Vec<CssKeyframe> {
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
