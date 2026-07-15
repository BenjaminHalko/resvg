// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::super::super::geom::{GeometryBake, ShapeGeometry, bake_geometry_animation};
use super::super::scanner::{matches_keyword, skip_string, skip_ws_comments};
use super::timing::parse_timing_function;
use crate::NormalizedF32;
use crate::tree::animation::{Accumulate, CalcMode};

pub(super) fn build_css_path_track(entries: &[(f32, &str, Option<&str>)]) -> Option<GeometryBake> {
    let mut d_keyframes = Vec::with_capacity(entries.len());
    let mut offsets = Vec::with_capacity(entries.len());
    let mut key_timing_fns = Vec::with_capacity(entries.len());
    for (offset, value, timing) in entries {
        d_keyframes.push(parse_css_path(value)?);
        offsets.push(NormalizedF32::new_clamped(*offset));
        key_timing_fns.push(timing.and_then(parse_timing_function));
    }

    bake_geometry_animation(
        crate::parser::svgtree::EId::Path,
        "d",
        ShapeGeometry::default(),
        &[],
        &offsets,
        &key_timing_fns,
        CalcMode::Linear,
        Accumulate::None,
        Some(&d_keyframes),
        None,
    )
}

fn parse_css_path(value: &str) -> Option<&str> {
    let value = value.trim();
    let bytes = value.as_bytes();
    if !matches_keyword(bytes, 0, b"path") {
        return None;
    }

    let mut index = skip_ws_comments(bytes, "path".len());
    if bytes.get(index) != Some(&b'(') {
        return None;
    }
    index = skip_ws_comments(bytes, index + 1);
    let quote = *bytes.get(index)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }

    let content_start = index + 1;
    let string_end = skip_string(bytes, index, quote);
    if string_end <= content_start || bytes.get(string_end - 1) != Some(&quote) {
        return None;
    }

    index = skip_ws_comments(bytes, string_end);
    if bytes.get(index) != Some(&b')') {
        return None;
    }
    index = skip_ws_comments(bytes, index + 1);
    (index == bytes.len()).then_some(&value[content_start..string_end - 1])
}
