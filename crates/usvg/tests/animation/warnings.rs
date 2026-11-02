// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use usvg::{AnimationFidelity, AnimationWarningCode};

use super::*;

fn codes(tree: &Tree) -> Vec<(AnimationWarningCode, Option<&str>, Option<&str>)> {
    tree.animation_warnings()
        .iter()
        .map(|warning| (warning.code(), warning.element(), warning.detail()))
        .collect()
}

#[test]
fn a_dropped_css_property_names_its_element_and_property() {
    let tree = parse(
        "<style>@keyframes shift { from { color: red; } to { color: blue; } } #box { animation: shift 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );

    assert_eq!(
        codes(&tree),
        [(
            AnimationWarningCode::UnsupportedCssProperty,
            Some("box"),
            Some("color")
        )]
    );
    assert_eq!(
        tree.animation_warnings()[0].fidelity(),
        AnimationFidelity::Dropped
    );
}

#[test]
fn a_css_variable_names_the_property_it_sits_in() {
    let tree = parse(
        "<style>@keyframes shift { from { fill: var(--a); } to { fill: var(--b); } } #box { animation: shift 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );

    assert_eq!(
        codes(&tree),
        [(AnimationWarningCode::CssVariable, Some("box"), Some("fill"))]
    );
    assert_eq!(
        tree.animation_warnings()[0].to_string(),
        "CSS variables are not supported."
    );
}

#[test]
fn an_ignored_timing_attribute_is_approximate() {
    let tree = parse(
        "<rect id='r' width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s' min='2s'/></rect>",
    );

    assert_eq!(
        codes(&tree),
        [(
            AnimationWarningCode::UnsupportedTimingAttribute,
            Some("r"),
            Some("min")
        )]
    );
    assert_eq!(
        tree.animation_warnings()[0].fidelity(),
        AnimationFidelity::Approximate
    );
}

#[test]
fn an_event_begin_is_named_once_though_the_node_is_read_more_than_once() {
    let tree = parse(
        "<rect id='r' width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s' begin='click'/></rect>",
    );

    assert_eq!(
        codes(&tree),
        [(
            AnimationWarningCode::UnsupportedBeginEnd,
            Some("r"),
            Some("click")
        )]
    );
}

#[test]
fn text_animation_is_dropped_and_named() {
    let tree = parse(
        "<text id='t' x='0' y='10'>Hi<animate attributeName='opacity' from='0' to='1' dur='1s'/></text>",
    );

    assert_eq!(
        codes(&tree),
        [(AnimationWarningCode::TextAnimation, Some("t"), None)]
    );
}

#[test]
fn an_element_without_an_id_is_named_by_no_id() {
    let tree = parse(
        "<rect width='4' height='4'><animate attributeName='letter-spacing' from='0' to='1' dur='1s'/></rect>",
    );

    assert_eq!(
        codes(&tree),
        [(
            AnimationWarningCode::UnsupportedAttribute,
            None,
            Some("letter-spacing")
        )]
    );
}

#[test]
fn a_still_svg_has_no_warnings() {
    let tree = parse("<rect width='4' height='4'/>");

    assert!(tree.animation_warnings().is_empty());
}

#[test]
fn fill_and_stroke_opacity_are_typed_tracks_not_losses() {
    let tree = parse(
        "<style>@keyframes f { from { stroke-opacity: 1 } to { stroke-opacity: 0 } } #r { animation: f 1s; }</style><rect id='r' width='4' height='4' fill='blue' stroke='red'><animate attributeName='fill-opacity' values='0;1' dur='1s'/></rect>",
    );

    assert!(tree.animation_warnings().is_empty());
    let kinds = path(&tree.root().children()[0])
        .animations()
        .iter()
        .map(|animation| match animation.kind() {
            AnimationKind::FillOpacity(_) => "fill-opacity",
            AnimationKind::StrokeOpacity(_) => "stroke-opacity",
            _ => "other",
        })
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["fill-opacity", "stroke-opacity"]);
}

#[test]
fn a_stop_colour_animated_to_a_value_starts_from_its_own() {
    let tree = parse(
        "<linearGradient id='g'><stop offset='0' stop-color='#000'><animate attributeName='stop-color' to='#fff' dur='1s'/></stop><stop offset='1' stop-color='#f00'/></linearGradient><rect width='5' height='5' fill='url(#g)'/>",
    );

    assert!(tree.linear_gradients()[0].animation().is_some());
}
