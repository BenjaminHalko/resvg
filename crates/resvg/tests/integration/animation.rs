// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Programmatic (no reference PNG) render tests for `resvg::render_at`.

use crate::{render_at_pixmap, render_pixmap};

/// The alpha of a single pixel, or `0` when out of bounds.
fn alpha_at(pixmap: &tiny_skia::Pixmap, x: u32, y: u32) -> u8 {
    pixmap.pixel(x, y).map(|p| p.alpha()).unwrap_or(0)
}

/// The inclusive `(min_x, min_y, max_x, max_y)` bounds of non-transparent pixels.
fn nonzero_bbox(pixmap: &tiny_skia::Pixmap) -> Option<(u32, u32, u32, u32)> {
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    for y in 0..pixmap.height() {
        for x in 0..pixmap.width() {
            if alpha_at(pixmap, x, y) > 0 {
                bbox = Some(match bbox {
                    Some((min_x, min_y, max_x, max_y)) => {
                        (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                    }
                    None => (x, y, x, y),
                });
            }
        }
    }
    bbox
}

#[test]
fn render_at_zero_matches_static_baseline() {
    // A static SVG carries no animations: `render_at(0)` must be byte-identical
    // to `render`, proving the animated path leaves static content untouched.
    let svg = r#"<svg width="60" height="60" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">
        <rect x="10" y="10" width="40" height="40" fill="green"/>
    </svg>"#;
    let baseline = render_pixmap(svg);
    let at_zero = render_at_pixmap(svg, 0.0);
    assert_eq!(baseline.data(), at_zero.data());
}

#[test]
fn nonfinite_time_is_treated_as_zero() {
    // NaN and infinities collapse to 0.0 and never panic.
    let svg = r#"<svg width="60" height="60" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">
        <rect x="10" y="10" width="40" height="40" fill="green">
            <animate attributeName="opacity" from="1" to="0" begin="0s" dur="4s" fill="freeze"/>
        </rect>
    </svg>"#;
    let zero = render_at_pixmap(svg, 0.0);
    for time in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(render_at_pixmap(svg, time).data(), zero.data());
    }
}

#[test]
fn inline_animated_transform_shifts_bbox() {
    // A rect with an inline `animateTransform` translate: the non-transparent
    // bbox moves right, its sign matching the positive track delta.
    let svg = r#"<svg width="120" height="60" viewBox="0 0 120 60" xmlns="http://www.w3.org/2000/svg">
        <rect x="10" y="10" width="20" height="20" fill="red">
            <animateTransform attributeName="transform" type="translate"
                from="0 0" to="40 0" begin="0s" dur="4s" fill="freeze"/>
        </rect>
    </svg>"#;
    let t0 = nonzero_bbox(&render_at_pixmap(svg, 0.0)).expect("content at t=0");
    let mid = nonzero_bbox(&render_at_pixmap(svg, 2.0)).expect("content at t=mid");
    let shift = mid.0 as i32 - t0.0 as i32;
    assert!((shift - 20).abs() <= 2, "expected ~+20px shift, got {shift}");
}

#[test]
fn wrapper_transform_offsets_bbox() {
    // A group-wrapped shape with an animated translate: at half time the bbox is
    // offset by the interpolated translation (~20px) within 1px.
    let svg = r#"<svg width="120" height="60" viewBox="0 0 120 60" xmlns="http://www.w3.org/2000/svg">
        <g>
            <animateTransform attributeName="transform" type="translate"
                from="0 0" to="40 0" begin="0s" dur="4s" fill="freeze"/>
            <rect x="10" y="10" width="20" height="20" fill="blue"/>
        </g>
    </svg>"#;
    let t0 = nonzero_bbox(&render_at_pixmap(svg, 0.0)).expect("content at t=0");
    let mid = nonzero_bbox(&render_at_pixmap(svg, 2.0)).expect("content at t=mid");
    let shift = mid.0 as i32 - t0.0 as i32;
    assert!((shift - 20).abs() <= 1, "expected ~+20px offset, got {shift}");
}

#[test]
fn animated_opacity_forces_isolation() {
    // An animated `opacity` on a rect whose static wrapper opacity is 1: a known
    // interior pixel's alpha drops between t=0 and t=mid, proving forced
    // isolation applies the sampled opacity.
    let svg = r#"<svg width="60" height="60" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">
        <rect x="10" y="10" width="40" height="40" fill="green">
            <animate attributeName="opacity" from="1" to="0" begin="0s" dur="4s" fill="freeze"/>
        </rect>
    </svg>"#;
    let a0 = alpha_at(&render_at_pixmap(svg, 0.0), 30, 30);
    let mid = alpha_at(&render_at_pixmap(svg, 2.0), 30, 30);
    assert_eq!(a0, 255, "fully opaque at t=0");
    assert!(mid > 0 && mid < a0, "opacity 0.5 reduces alpha, got {mid}");
    assert!((mid as i16 - 128).abs() <= 8, "expected ~128 alpha, got {mid}");
}

#[test]
fn animated_dashoffset_toggles_pixel() {
    // A dashed stroke with an animated `stroke-dashoffset`: a pixel that sits in
    // a gap at t=0 is covered at t=mid (a half-period shift inverts coverage).
    let svg = r#"<svg width="100" height="20" viewBox="0 0 100 20" xmlns="http://www.w3.org/2000/svg">
        <line x1="0" y1="10" x2="100" y2="10" stroke="black" stroke-width="10"
            stroke-dasharray="10 10">
            <animate attributeName="stroke-dashoffset" from="0" to="10" begin="0s" dur="2s" fill="freeze"/>
        </line>
    </svg>"#;
    let a0 = alpha_at(&render_at_pixmap(svg, 0.0), 15, 10);
    let mid = alpha_at(&render_at_pixmap(svg, 2.0), 15, 10);
    assert!(
        (a0 as i16 - mid as i16).abs() > 100,
        "dash coverage must toggle, {a0} vs {mid}"
    );
}

#[test]
fn grow_from_zero_renders_interpolated_width() {
    // A rect grown from width 0 to 100: empty at t=0, an interpolated ~50px at
    // t=mid (never 0, never the full 100).
    let svg = r#"<svg width="120" height="60" viewBox="0 0 120 60" xmlns="http://www.w3.org/2000/svg">
        <rect x="10" y="10" width="0" height="30" fill="purple">
            <animate attributeName="width" from="0" to="100" begin="0s" dur="4s" fill="freeze"/>
        </rect>
    </svg>"#;
    assert!(
        nonzero_bbox(&render_at_pixmap(svg, 0.0)).is_none(),
        "zero width renders nothing at t=0"
    );
    let mid = nonzero_bbox(&render_at_pixmap(svg, 2.0)).expect("content at t=mid");
    let width = mid.2 - mid.0 + 1;
    assert!((width as i32 - 50).abs() <= 2, "expected ~50px width, got {width}");
}

#[test]
fn paint_carrier_reveals_fill_and_stroke() {
    // Fill carrier: `fill="none"` paints nothing statically; an animated color
    // fills the interior under `render_at`.
    let fill_svg = r#"<svg width="60" height="60" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">
        <path d="M10 10 H50 V50 H10 Z" fill="none">
            <animate attributeName="fill" from="red" to="blue" begin="0s" dur="4s" fill="freeze"/>
        </path>
    </svg>"#;
    assert!(
        nonzero_bbox(&render_pixmap(fill_svg)).is_none(),
        "static fill=none paints nothing"
    );
    assert!(
        alpha_at(&render_at_pixmap(fill_svg, 2.0), 30, 30) > 200,
        "animated fill covers the interior"
    );

    // Stroke carrier: `stroke-width="0"` paints nothing statically; an animated
    // width reveals the stroke.
    let stroke_svg = r#"<svg width="60" height="60" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">
        <path d="M10 30 H50" stroke="black" stroke-width="0" fill="none">
            <animate attributeName="stroke-width" from="0" to="10" begin="0s" dur="4s" fill="freeze"/>
        </path>
    </svg>"#;
    assert_eq!(
        alpha_at(&render_pixmap(stroke_svg), 30, 30),
        0,
        "static stroke-width=0 paints nothing"
    );
    assert!(
        alpha_at(&render_at_pixmap(stroke_svg, 2.0), 30, 30) > 200,
        "animated stroke width reveals the stroke"
    );
}

#[test]
fn fill_carrier_preserves_opacity_and_rule() {
    // A `fill="none"` carrier still holds fill-opacity and fill-rule. An animated
    // color renders with both preserved: the self-overlapping path leaves an
    // even-odd hole at its center, and the ring paints at ~0.5 alpha.
    let svg = r#"<svg width="60" height="60" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">
        <path d="M10 10 H50 V50 H10 Z M20 20 H40 V40 H20 Z" fill="none"
            fill-opacity="0.5" fill-rule="evenodd">
            <animate attributeName="fill" from="red" to="blue" begin="0s" dur="4s" fill="freeze"/>
        </path>
    </svg>"#;
    let pixmap = render_at_pixmap(svg, 2.0);
    let ring = alpha_at(&pixmap, 15, 30);
    assert!((ring as i16 - 128).abs() <= 12, "expected ~128 ring alpha, got {ring}");
    let center = alpha_at(&pixmap, 30, 30);
    assert!(center < 20, "even-odd center must be a hole, got {center}");
}

/// A 10x10 solid green raster stand-in, embedded as an SVG data URI.
const GREEN_IMAGE: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMCIgaGVpZ2h0PSIxMCI+PHJlY3Qgd2lkdGg9IjEwIiBoZWlnaHQ9IjEwIiBmaWxsPSJncmVlbiIvPjwvc3ZnPg==";

/// A 10x10 image split red (left half) and blue (right half), as a data URI.
const SPLIT_IMAGE: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMCIgaGVpZ2h0PSIxMCI+PHJlY3QgeD0iMCIgeT0iMCIgd2lkdGg9IjUiIGhlaWdodD0iMTAiIGZpbGw9InJlZCIvPjxyZWN0IHg9IjUiIHk9IjAiIHdpZHRoPSI1IiBoZWlnaHQ9IjEwIiBmaWxsPSJibHVlIi8+PC9zdmc+";

#[test]
fn animated_root_view_box_pans_content() {
    // A `viewBox` panning its origin right shifts rendered content left: the
    // rect's left edge moves from x=40 toward x=0 as the origin advances to 40.
    let svg = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
        <animate attributeName="viewBox" from="0 0 100 100" to="40 0 100 100"
            begin="0s" dur="4s" fill="freeze"/>
        <rect x="40" y="40" width="20" height="20" fill="green"/>
    </svg>"#
    );
    let t0 = nonzero_bbox(&render_at_pixmap(&svg, 0.0)).expect("content at t=0");
    let end = nonzero_bbox(&render_at_pixmap(&svg, 4.0)).expect("content at t=end");
    assert!((t0.0 as i32 - 40).abs() <= 2, "left edge starts near x=40, got {}", t0.0);
    let shift = end.0 as i32 - t0.0 as i32;
    assert!((shift + 40).abs() <= 2, "view box pan shifts content ~-40px, got {shift}");
}

#[test]
fn animated_slice_image_scales_and_reclips() {
    // A red|blue split image sliced into a widening rectangle: the clipped region
    // grows to the right and the image rescales, moving the color boundary.
    let svg = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100"
        xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
        <image x="10" y="10" width="20" height="40" preserveAspectRatio="xMidYMid slice"
            xlink:href="{SPLIT_IMAGE}">
            <animate attributeName="width" from="20" to="80" begin="0s" dur="4s" fill="freeze"/>
        </image>
    </svg>"#
    );
    let t0 = render_at_pixmap(&svg, 0.0);
    let mid = render_at_pixmap(&svg, 2.0);
    let bbox0 = nonzero_bbox(&t0).expect("content at t=0");
    let bbox_mid = nonzero_bbox(&mid).expect("content at t=mid");
    let width0 = bbox0.2 - bbox0.0 + 1;
    let width_mid = bbox_mid.2 - bbox_mid.0 + 1;
    assert!(width_mid > width0 + 10, "slice clip widens, {width0} -> {width_mid}");
    // A pixel outside the t=0 clip becomes covered once the slice widens.
    assert_eq!(alpha_at(&t0, 45, 30), 0, "pixel outside the t=0 slice is empty");
    assert!(alpha_at(&mid, 45, 30) > 200, "the widened slice covers that pixel");
    // The rescale moves the red|blue boundary, flipping a fixed interior pixel.
    let c0 = t0.pixel(25, 30).unwrap();
    let c_mid = mid.pixel(25, 30).unwrap();
    assert!(
        c0.red().abs_diff(c_mid.red()) > 100 || c0.blue().abs_diff(c_mid.blue()) > 100,
        "image rescale flips the (25,30) hue"
    );
}

#[test]
fn partial_image_animation_keeps_static_dims() {
    // Animating only `width` leaves x, y, and height at their static values: at
    // t=mid the square image fills the 40x40 rect anchored at (10,10).
    let svg = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100"
        xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
        <image x="10" y="10" width="20" height="40" xlink:href="{GREEN_IMAGE}">
            <animate attributeName="width" from="20" to="60" begin="0s" dur="4s" fill="freeze"/>
        </image>
    </svg>"#
    );
    let bbox = nonzero_bbox(&render_at_pixmap(&svg, 2.0)).expect("content at t=mid");
    assert!((bbox.0 as i32 - 10).abs() <= 2, "static x preserved, got {}", bbox.0);
    assert!((bbox.1 as i32 - 10).abs() <= 2, "static y preserved, got {}", bbox.1);
    let height = bbox.3 - bbox.1 + 1;
    assert!((height as i32 - 40).abs() <= 3, "static height preserved, got {height}");
}

#[test]
fn image_width_crossing_zero_renders_nothing() {
    // `width` dipping to 0 at the mid keyframe hides the image without panicking,
    // and it reappears on either side of the crossing.
    let svg = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100"
        xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
        <image x="10" y="10" width="40" height="40" xlink:href="{GREEN_IMAGE}">
            <animate attributeName="width" values="40;0;40" begin="0s" dur="4s" fill="freeze"/>
        </image>
    </svg>"#
    );
    assert!(
        nonzero_bbox(&render_at_pixmap(&svg, 2.0)).is_none(),
        "zero width renders nothing at the crossing"
    );
    assert!(
        nonzero_bbox(&render_at_pixmap(&svg, 0.0)).is_some(),
        "the image renders on either side of the crossing"
    );
}

#[test]
fn zero_static_image_reveals_under_render_at() {
    // A statically zero-width image is a placeholder: `render` paints nothing,
    // while `render_at` reveals the interpolated width.
    let svg = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100"
        xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
        <image x="10" y="10" width="0" height="40" xlink:href="{GREEN_IMAGE}">
            <animate attributeName="width" from="0" to="80" begin="0s" dur="4s" fill="freeze"/>
        </image>
    </svg>"#
    );
    assert!(
        nonzero_bbox(&render_pixmap(&svg)).is_none(),
        "the zero-width placeholder paints nothing under render()"
    );
    let mid = nonzero_bbox(&render_at_pixmap(&svg, 2.0)).expect("render_at reveals the image");
    let width = mid.2 - mid.0 + 1;
    assert!(width > 10, "the interpolated width renders, got {width}");
}
