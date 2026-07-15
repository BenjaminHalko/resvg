// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn css_d_path_keyframes_create_replacing_path_track_when_verbs_match() {
    // Given: CSS path() keyframes whose SVG path verbs match.
    let svg = "<style>@keyframes morph { from { d: path('M0 0 L10 0'); } to { d: path('M0 0 L20 10'); } } #shape { animation: morph 1s linear; }</style><path id='shape' d='M0 0 L10 0'/>";

    let (tree, warnings) = {
        let _guard = WARN_GUARD.lock().unwrap();
        init_capture();
        WARNINGS.get().unwrap().lock().unwrap().clear();

        // When: the parser lowers the CSS animation into the public tree model.
        let tree = parse(svg);
        let warnings = WARNINGS.get().unwrap().lock().unwrap().clone();
        (tree, warnings)
    };

    // Then: the property is supported and retains replacing path geometry.
    assert!(
        !warnings
            .iter()
            .any(|warning| warning == "Unsupported CSS property in keyframes: 'd'."),
        "CSS d must not be rejected as unsupported: {warnings:?}"
    );
    let node_animation = path(&tree.root().children()[0]).animation().unwrap();
    assert_eq!(node_animation.animations().len(), 1);
    let animation = &node_animation.animations()[0];
    assert!(matches!(animation.source(), AnimationSource::Css));
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a CSS path track");
    };
    assert_eq!(track.keyframes().len(), 2);
    assert_eq!(track.keyframes()[0].offset().get(), 0.0);
    assert_eq!(track.keyframes()[1].offset().get(), 1.0);
    assert!(track.replaces_geometry());
    assert_eq!(
        track.keyframes()[0].path().verbs(),
        track.keyframes()[1].path().verbs()
    );
    let start_points = track.keyframes()[0].path().points();
    assert_eq!(start_points.len(), 2);
    assert_eq!((start_points[0].x, start_points[0].y), (0.0, 0.0));
    assert_eq!((start_points[1].x, start_points[1].y), (10.0, 0.0));
    let end_points = track.keyframes()[1].path().points();
    assert_eq!(end_points.len(), 2);
    assert_eq!((end_points[0].x, end_points[0].y), (0.0, 0.0));
    assert_eq!((end_points[1].x, end_points[1].y), (20.0, 10.0));
}

#[test]
fn css_d_path_keyframes_keep_animation_timing_function() {
    // Given: CSS path() keyframes with an animation-level timing function.
    let svg = "<style>@keyframes morph { from { d: path('M0 0 L10 0'); } to { d: path('M0 0 L20 10'); } } #shape { animation: morph 1s ease-in-out; }</style><path id='shape' d='M0 0 L10 0'/>";

    // When: the parser lowers the CSS animation into the public tree model.
    let tree = parse(svg);
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];

    // Then: the path track retains the animation-level easing.
    assert!(matches!(
        animation.easing().timing_function(),
        Some(TimingFunction::CubicBezier(x1, y1, x2, y2))
            if *x1 == 0.42 && *y1 == 0.0 && *x2 == 0.58 && *y2 == 1.0
    ));
}

#[test]
fn css_d_path_keyframes_keep_per_keyframe_timing_function() {
    // Given: a CSS path() keyframe with a property-local timing function.
    let svg = "<style>@keyframes morph { 0% { d: path('M0 0 L10 0'); animation-timing-function: steps(4, jump-end); } 100% { d: path('M0 0 L20 10'); } } #shape { animation: morph 1s ease-in-out; }</style><path id='shape' d='M0 0 L10 0'/>";

    // When: the parser lowers the CSS path animation into the public tree model.
    let tree = parse(svg);
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a CSS path track");
    };

    // Then: global and property-local easing metadata remain independently available.
    assert!(matches!(
        animation.easing().timing_function(),
        Some(TimingFunction::CubicBezier(x1, y1, x2, y2))
            if *x1 == 0.42 && *y1 == 0.0 && *x2 == 0.58 && *y2 == 1.0
    ));
    assert!(matches!(
        track.keyframes()[0].timing_function(),
        Some(TimingFunction::Steps(4, StepPosition::JumpEnd))
    ));
}

#[test]
fn css_d_path_keyframes_accept_double_quoted_path_data() {
    // Given: CSS keyframes whose path() data uses double quotes.
    let svg = r#"<style>@keyframes morph { from { d: path("M0 0 L10 0"); } to { d: path("M0 0 L20 0"); } } #shape { animation: morph 1s linear; }</style><path id="shape" d="M0 0 L10 0"/>"#;

    // When: the parser lowers the CSS keyframes.
    let tree = parse(svg);

    // Then: it produces the same replacing CSS path track.
    let node_animation = path(&tree.root().children()[0]).animation().unwrap();
    let animation = &node_animation.animations()[0];
    assert!(matches!(animation.source(), AnimationSource::Css));
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a CSS path track");
    };
    assert_eq!(track.keyframes().len(), 2);
    assert!(track.replaces_geometry());
}

#[test]
fn css_d_path_keyframes_use_discrete_interpolation_when_verbs_differ() {
    // Given: CSS path() keyframes with incompatible SVG path verbs.
    let svg = "<style>@keyframes morph { from { d: path('M0 0 L10 10'); } to { d: path('M0 0 L10 10 L20 0'); } } #shape { animation: morph 1s linear; }</style><path id='shape' d='M0 0 L10 10'/>";

    let (tree, warnings) = {
        let _guard = WARN_GUARD.lock().unwrap();
        init_capture();
        WARNINGS.get().unwrap().lock().unwrap().clear();

        // When: the parser lowers the CSS animation into the public tree model.
        let tree = parse(svg);
        let warnings = WARNINGS.get().unwrap().lock().unwrap().clone();
        (tree, warnings)
    };

    // Then: the existing path geometry fallback selects discrete interpolation.
    assert!(
        !warnings
            .iter()
            .any(|warning| warning == "Unsupported CSS property in keyframes: 'd'."),
        "CSS d must not be rejected as unsupported: {warnings:?}"
    );
    let node_animation = path(&tree.root().children()[0]).animation().unwrap();
    assert_eq!(node_animation.animations().len(), 1);
    let animation = &node_animation.animations()[0];
    assert!(matches!(animation.source(), AnimationSource::Css));
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a CSS path track");
    };
    assert_eq!(track.keyframes().len(), 2);
    assert!(track.replaces_geometry());
    assert_ne!(
        track.keyframes()[0].path().verbs(),
        track.keyframes()[1].path().verbs()
    );
    assert!(matches!(animation.easing().calc_mode(), CalcMode::Discrete));
}

#[test]
fn css_d_path_keyframes_reject_unquoted_and_trailing_path_functions() {
    // Given: CSS d keyframes with unquoted or trailing path() syntax.
    let svg = "<style>@keyframes unquoted { from { d: path(M0 0 L10 0); } to { d: path(M0 0 L20 0); } } @keyframes trailing { from { d: path('M0 0 L10 0') extra; } to { d: path('M0 0 L20 0') extra; } } #unquoted { animation: unquoted 1s linear; } #trailing { animation: trailing 1s linear; }</style><path id='unquoted' d='M0 0 L10 0'/><path id='trailing' d='M0 0 L10 0'/>";

    // When: the parser receives the invalid CSS values.
    let tree = parse(svg);

    // Then: neither path receives an animation track.
    assert!(path(&tree.root().children()[0]).animation().is_none());
    assert!(path(&tree.root().children()[1]).animation().is_none());
}

#[test]
fn css_d_path_keyframes_reject_prefix_valid_quoted_path_data() {
    // Given: quoted CSS path() keyframes with an incomplete final command.
    let svg = "<style>@keyframes malformed { from { d: path('M0 0 L10 0 Q'); } to { d: path('M0 0 L20 0 Q'); } } #shape { animation: malformed 1s linear; }</style><path id='shape' d='M0 0 L10 0'/>";

    // When: the parser lowers the malformed keyframes.
    let tree = {
        let _guard = WARN_GUARD.lock().unwrap();
        init_capture();
        WARNINGS.get().unwrap().lock().unwrap().clear();
        parse(svg)
    };

    // Then: the prefix must not become a CSS path animation.
    assert!(path(&tree.root().children()[0]).animation().is_none());
}

#[test]
fn css_d_path_keyframes_do_not_attach_to_rect_targets() {
    // Given: an otherwise valid CSS d keyframe animation on a rectangle.
    let svg = "<style>@keyframes morph { from { d: path('M0 0 L10 0'); } to { d: path('M0 0 L20 0'); } } #shape { animation: morph 1s linear; }</style><rect id='shape' width='10' height='10'/>";

    // When: the parser lowers the rectangle.
    let tree = {
        let _guard = WARN_GUARD.lock().unwrap();
        init_capture();
        WARNINGS.get().unwrap().lock().unwrap().clear();
        parse(svg)
    };

    // Then: only path targets can receive CSS d animation tracks.
    assert!(path(&tree.root().children()[0]).animation().is_none());
}
