// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn linear_gradient_endpoints() {
    // Given: only x1 is animated, from 0 to 100 in user space.
    let animated = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="15" x2="100" y2="85"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="x1" from="0" to="100" dur="1s" fill="freeze"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="50" y1="15" x2="100" y2="85"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;

    // When: the midpoint is rendered.
    let actual = render_at_pixmap(animated, 0.5);

    // Then: x1 is 50 while y1, x2, and y2 retain their static values.
    assert_eq!(actual.data(), render_pixmap(expected).data());
}

#[test]
fn radial_component_identity() {
    // Given: cx, cy, fx, and fy start at the same scalar value.
    let animated = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="50" cy="50" fx="50" fy="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="fy" from="50" to="75" dur="1s" fill="freeze"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="50" cy="50" fx="50" fy="75" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;

    // When: fy reaches its frozen endpoint.
    let actual = render_at_pixmap(animated, 1.0);

    // Then: only fy changes.
    assert_eq!(actual.data(), render_pixmap(expected).data());
}

#[test]
fn radial_focal_omission() {
    // Given: one radial gradient omits fx and another explicitly supplies its
    // initial center value as fx.
    let omitted = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="50" cy="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="cx" from="50" to="75" dur="1s" fill="freeze"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let omitted_expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="75" cy="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let explicit = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="50" cy="50" fx="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="cx" from="50" to="75" dur="1s" fill="freeze"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let explicit_expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="75" cy="50" fx="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;

    // When: cx reaches its endpoint.
    let omitted_actual = render_at_pixmap(omitted, 1.0);
    let explicit_actual = render_at_pixmap(explicit, 1.0);

    // Then: only the omitted focal x follows the animated center.
    assert_eq!(omitted_actual.data(), render_pixmap(omitted_expected).data());
    assert_eq!(explicit_actual.data(), render_pixmap(explicit_expected).data());
}

#[test]
fn radial_focal_href_explicit() {
    // Given: fx is explicitly inherited through href, even though it equals cx.
    let animated = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="base" gradientUnits="userSpaceOnUse" cx="50" cy="50" fx="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient><radialGradient id="g" href="#base"><animate attributeName="cx" from="50" to="75" dur="1s" fill="freeze"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"##;
    let expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="75" cy="50" fx="50" r="40"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;

    // When: the inherited gradient's center reaches its endpoint.
    let actual = render_at_pixmap(animated, 1.0);

    // Then: inherited explicit fx remains independent of cx.
    assert_eq!(actual.data(), render_pixmap(expected).data());
}

#[test]
fn gradient_units_resolution() {
    // Given: the same percentage endpoint in objectBoundingBox and userSpaceOnUse.
    let bbox_animated = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" x1="0" y1="0" x2="100%" y2="0"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="x1" from="0" to="100%" dur="1s" fill="freeze"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let bbox_expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" x1="50%" y1="0" x2="100%" y2="0"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let user_animated = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="100" y2="0"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="x1" from="0" to="100%" dur="1s" fill="freeze"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;
    let user_expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="50%" y1="0" x2="100" y2="0"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;

    // When: both midpoint samples are rendered.
    let bbox_actual = render_at_pixmap(bbox_animated, 0.5);
    let user_actual = render_at_pixmap(user_animated, 0.5);

    // Then: both unit systems use their static converter's coordinate space.
    assert_eq!(bbox_actual.data(), render_pixmap(bbox_expected).data());
    assert_eq!(user_actual.data(), render_pixmap(user_expected).data());
}

#[test]
fn radial_nonpositive_carrier() {
    // Given: an animated radial radius crosses zero while preserving its carrier.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="50" cy="50" r="-10"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/><animate attributeName="r" from="-10" to="10" dur="1s" fill="freeze"/></radialGradient></defs><rect width="100" height="100" fill="url(#g)"/></svg>"#;

    // When: sampling before and after the non-positive crossing.
    let before = rgb_at(&render_at_pixmap(svg, 0.25), 50, 50);
    let after = rgb_at(&render_at_pixmap(svg, 0.75), 50, 50);

    // Then: non-positive r is a solid last stop, while positive r restores the gradient.
    assert!(before.2 > 200 && before.0 < 60, "expected last-stop blue, got {before:?}");
    assert!(after.0 > 200 && after.2 < 60, "expected first-stop red, got {after:?}");
}
