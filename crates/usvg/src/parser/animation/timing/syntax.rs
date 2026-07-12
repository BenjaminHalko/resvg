// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::clock::parse_offset;
use super::{RawBegin, SyncEdge};

/// Parses a single `begin`/`end` list entry into a [`RawBegin`].
///
/// Offsets and `indefinite` resolve immediately; syncbase values are kept for
/// topological resolution. Event-based and otherwise unsupported values return
/// `None`.
pub(super) fn parse_begin_entry(entry: &str) -> Option<RawBegin> {
    let entry = entry.trim();
    if entry.is_empty() {
        return None;
    }
    if entry == "indefinite" {
        return Some(RawBegin::Indefinite);
    }

    let first = entry.as_bytes()[0];
    if first == b'+' || first == b'-' || first == b'.' || first.is_ascii_digit() {
        return parse_offset(entry).map(RawBegin::Offset);
    }

    parse_syncbase(entry)
}

/// Parses a syncbase value of the form `id.begin`/`id.end` with an optional
/// offset.
fn parse_syncbase(entry: &str) -> Option<RawBegin> {
    for (edge, marker) in [(SyncEdge::Begin, ".begin"), (SyncEdge::End, ".end")] {
        let mut search = 0;
        while let Some(pos) = entry[search..].find(marker) {
            let index = search + pos;
            let id = &entry[..index];
            let rest = &entry[index + marker.len()..];
            if !id.is_empty() {
                if let Some(offset) = parse_sync_offset(rest) {
                    return Some(RawBegin::SyncBase {
                        id: id.to_string(),
                        edge,
                        offset,
                    });
                }
            }
            search = index + 1;
        }
    }
    None
}

/// Parses the trailing offset of a syncbase value, or `0.0` when absent.
fn parse_sync_offset(rest: &str) -> Option<f32> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(0.0);
    }
    if !rest.starts_with('+') && !rest.starts_with('-') {
        return None;
    }
    parse_offset(rest)
}
