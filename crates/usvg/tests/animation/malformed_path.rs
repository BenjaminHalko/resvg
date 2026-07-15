// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn smil_d_path_animation_rejects_prefix_valid_path_data() {
    // Given: SMIL d values with an incomplete final SVG path command.
    let svg = "<path d='M0 0 L10 0'><animate attributeName='d' values='M0 0 L10 0 Q;M0 0 L20 0 Q' dur='1s'/></path>";

    // When: the parser bakes the geometry animation.
    let tree = {
        let _guard = WARN_GUARD.lock().unwrap();
        init_capture();
        WARNINGS.get().unwrap().lock().unwrap().clear();
        parse(svg)
    };

    // Then: the valid prefix must not produce a path animation.
    assert!(path(&tree.root().children()[0]).animation().is_none());
}
