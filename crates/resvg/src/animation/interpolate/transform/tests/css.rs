use tiny_skia::Transform;
use usvg::{Accumulate, Additive, AnimationKind, Node, Options, TransformFunction, Tree};

use super::common::{approx, approx_transform, linear, sample, track};

#[test]
fn x7_compatible_css_lists_lerp_elementwise() {
    // Given: compatible CSS function lists.
    let track = track(&[
        (
            0.0,
            vec![
                TransformFunction::Translate(0.0, 0.0),
                TransformFunction::Scale(1.0, 1.0),
            ],
        ),
        (
            1.0,
            vec![
                TransformFunction::Translate(10.0, 20.0),
                TransformFunction::Scale(3.0, 5.0),
            ],
        ),
    ]);

    // When: sampled halfway.
    let matrix = sample(&track, &linear(), 0.5);

    // Then: each function's parameters lerp independently.
    approx_transform(
        matrix,
        Transform::from_translate(5.0, 10.0).pre_concat(Transform::from_scale(2.0, 3.0)),
    );
}

#[test]
fn x8_incompatible_css_lists_step_by_source_offset() {
    // Given: incompatible lists at source offsets zero and one.
    let track = track(&[
        (0.0, vec![TransformFunction::Translate(10.0, 20.0)]),
        (1.0, vec![TransformFunction::Scale(2.0, 3.0)]),
    ]);

    // When: sampling before and at .5.
    let before = sample(&track, &linear(), 0.49);
    let at = sample(&track, &linear(), 0.5);

    // Then: offset-based fallback holds the translate at both positions.
    approx_transform(before, Transform::from_translate(10.0, 20.0));
    approx_transform(at, Transform::from_translate(10.0, 20.0));
    assert_eq!(
        super::super::INCOMPATIBLE_WARNING,
        "Unsupported transform animation; using discrete interpolation."
    );
}

#[test]
fn x8_offset_boundary_selects_the_second_incompatible_list() {
    // Given: the same incompatible signatures with the second source offset at .5.
    let track = track(&[
        (0.0, vec![TransformFunction::Translate(10.0, 20.0)]),
        (0.5, vec![TransformFunction::Scale(2.0, 3.0)]),
    ]);

    // When: sampled at the second keyframe's source offset.
    let matrix = sample(&track, &linear(), 0.5);

    // Then: the second list is selected exactly at its offset.
    approx_transform(matrix, Transform::from_scale(2.0, 3.0));
}

#[test]
fn x9_origin_bake_uses_stroke_box() {
    // Given: fill and stroke bounds that resolve a different percentage origin.
    let tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg'><style>@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(90deg); } } #box { transform-origin: 100% 50%; transform-box: stroke-box; animation: spin 1s linear; }</style><rect id='box' width='10' height='10' stroke='black' stroke-width='10'/></svg>",
        &Options::default(),
    )
    .unwrap_or_else(|error| panic!("CSS origin parse failed: {error:?}"));
    let Node::Group(group) = &tree.root().children()[0] else {
        panic!("expected CSS transform wrapper group");
    };
    let AnimationKind::Transform(track) = group.animations()[0].kind() else {
        panic!("expected transform track");
    };

    // When: the baked list is sampled halfway.
    let matrix = sample(track, &linear(), 0.5);

    // Then: 100% 50% resolves to stroke-box origin (15, 5), never fill-box (10, 5).
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [
            TransformFunction::Translate(x, y),
            TransformFunction::Rotate(_),
            TransformFunction::Translate(_, _),
        ] if *x == 15.0 && *y == 5.0
    ));
    approx(matrix.tx, 7.928932);
    approx_transform(
        matrix,
        Transform::from_translate(15.0, 5.0)
            .pre_concat(Transform::from_rotate(45.0))
            .pre_translate(-15.0, -5.0),
    );
}

#[test]
fn x9b_origin_bake_uses_default_bounds_for_non_stroke_boxes() {
    // Given: an offset fill box and a percentage CSS origin.
    let tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg'><style>@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(90deg); } } #box { transform-origin: 25% 75%; transform-box: fill-box; animation: spin 1s linear; }</style><rect id='box' x='2' y='3' width='20' height='40'/></svg>",
        &Options::default(),
    )
    .unwrap_or_else(|error| panic!("CSS origin parse failed: {error:?}"));
    let Node::Group(group) = &tree.root().children()[0] else {
        panic!("expected CSS transform wrapper group");
    };
    let AnimationKind::Transform(track) = group.animations()[0].kind() else {
        panic!("expected transform track");
    };

    // When: the CSS transform is sampled.
    let matrix = sample(track, &linear(), 0.5);

    // Then: fill-box origin resolves to (7, 33) before composition.
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [TransformFunction::Translate(x, y), ..] if *x == 7.0 && *y == 33.0
    ));
    approx_transform(
        matrix,
        Transform::from_translate(7.0, 33.0)
            .pre_concat(Transform::from_rotate(45.0))
            .pre_translate(-7.0, -33.0),
    );
}

#[test]
fn x9c_css_transform_is_replace_only_without_accumulation() {
    // Given: a parsed CSS transform animation.
    let tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg'><style>@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(90deg); } } #box { animation: spin 1s linear; }</style><rect id='box' width='4' height='4'/></svg>",
        &Options::default(),
    )
    .unwrap_or_else(|error| panic!("CSS transform parse failed: {error:?}"));
    let Node::Group(group) = &tree.root().children()[0] else {
        panic!("expected CSS transform wrapper group");
    };
    let animation = &group.animations()[0];

    // When: parser output is inspected after origin baking.
    let kind = animation.kind();

    // Then: distribution over origin wrappers is valid only under Replace/None.
    assert!(matches!(kind, AnimationKind::Transform(_)));
    assert!(matches!(animation.additive(), Additive::Replace));
    assert!(matches!(animation.accumulate(), Accumulate::None));
}

#[test]
fn css_origin_lengths_use_static_unit_resolution() {
    // Given: a CSS origin with an absolute physical length and a pixel length.
    let tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg'><style>@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(90deg); } } #box { transform-origin: 5mm 10px; animation: spin 1s linear; }</style><rect id='box' width='4' height='4'/></svg>",
        &Options::default(),
    )
    .unwrap_or_else(|error| panic!("CSS origin parse failed: {error:?}"));
    let Node::Group(group) = &tree.root().children()[0] else {
        panic!("expected CSS transform wrapper group");
    };
    let AnimationKind::Transform(track) = group.animations()[0].kind() else {
        panic!("expected transform track");
    };

    // When: post-bounds baking resolves the origin wrappers.
    let first = track.keyframes()[0].value();

    // Then: the non-percent component uses the standard 96dpi CSS conversion.
    assert!(matches!(
        first.as_slice(),
        [TransformFunction::Translate(x, y), ..]
            if (*x - 5.0 * 96.0 / 25.4).abs() <= 1e-4 && *y == 10.0
    ));
}
