// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Splits `text` on `delimiter` at the top level, ignoring delimiters inside
/// strings, parentheses and comments.
pub(super) fn split_top_level(text: &str, delimiter: u8) -> Vec<&str> {
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

/// Finds the index of the `}` matching the `{` at `open`, tracking nested braces
/// while ignoring braces inside strings and comments.
pub(super) fn find_block_end(bytes: &[u8], open: usize) -> Option<usize> {
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
pub(super) fn skip_at_statement(bytes: &[u8], at: usize) -> usize {
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

pub(super) fn skip_ws_comments(bytes: &[u8], mut i: usize) -> usize {
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
pub(super) fn skip_comment(bytes: &[u8], i: usize) -> usize {
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
pub(super) fn skip_string(bytes: &[u8], i: usize, quote: u8) -> usize {
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

pub(super) fn matches_keyword(bytes: &[u8], start: usize, keyword: &[u8]) -> bool {
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
