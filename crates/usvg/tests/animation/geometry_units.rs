// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn css_stroke_width_units() {
    for value in ["10", "10%", "5mm", "1em"] {
        // Given: a static length and an equivalent CSS keyframe value.
        let static_tree = parse(&format!(
            "<path d='M0 0 L4 4' stroke='black' stroke-width='{value}'/>"
        ));
        let expected = path(&static_tree.root().children()[0])
            .stroke()
            .unwrap()
            .width()
            .get();
        let animated_tree = parse(&format!(
            "<style>@keyframes grow {{ from {{ stroke-width: {value}; }} to {{ stroke-width: {value}; }} }} #line {{ animation: grow 1s linear; }}</style><path id='line' d='M0 0 L4 4' stroke='black'/>"
        ));
        let animation = &path(&animated_tree.root().children()[0])
            .animation()
            .unwrap()
            .animations()[0];
        let AnimationKind::StrokeWidth(track) = animation.kind() else {
            panic!("expected a stroke-width track");
        };

        // When: the CSS keyframe is parsed.
        let actual = *track.keyframes()[1].value();

        // Then: unitless 10 stays 10 and every length unit matches static resolution.
        assert_eq!(actual, expected, "stroke-width {value}");
    }
}

#[test]
fn css_dashoffset_units() {
    for value in ["10", "10%", "5mm", "1em"] {
        // Given: a static dash offset and an equivalent CSS keyframe value.
        let static_tree = parse(&format!(
            "<path d='M0 0 L4 4' stroke='black' stroke-dasharray='2 2' stroke-dashoffset='{value}'/>"
        ));
        let expected = path(&static_tree.root().children()[0])
            .stroke()
            .unwrap()
            .dashoffset();
        let animated_tree = parse(&format!(
            "<style>@keyframes shift {{ from {{ stroke-dashoffset: {value}; }} to {{ stroke-dashoffset: {value}; }} }} #line {{ animation: shift 1s linear; }}</style><path id='line' d='M0 0 L4 4' stroke='black' stroke-dasharray='2 2'/>",
        ));
        let animation = &path(&animated_tree.root().children()[0])
            .animation()
            .unwrap()
            .animations()[0];
        let AnimationKind::StrokeDashoffset(track) = animation.kind() else {
            panic!("expected a stroke-dashoffset track");
        };

        // When: the CSS keyframe is parsed.
        let actual = *track.keyframes()[1].value();

        // Then: unitless 10 stays 10 and every length unit matches static resolution.
        assert_eq!(actual, expected, "stroke-dashoffset {value}");
    }
}

#[test]
fn smil_shape_image_axes() {
    // Given: shape geometry uses a 200x100 viewport, so horizontal and vertical
    // percentages must resolve against different axes.
    let shape_tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><rect x='0' y='0' width='10' height='10'><animate attributeName='x' from='50%' to='50%' dur='1s'/><animate attributeName='y' from='50%' to='50%' dur='1s'/><animate attributeName='width' from='50%' to='50%' dur='1s'/><animate attributeName='height' from='50%' to='50%' dur='1s'/></rect></svg>",
        &Options::default(),
    )
    .unwrap();
    let animations = path(&shape_tree.root().children()[0])
        .animation()
        .unwrap()
        .animations();
    let AnimationKind::Path(x_track) = animations[0].kind() else {
        panic!("expected an x path track");
    };
    let AnimationKind::Path(y_track) = animations[1].kind() else {
        panic!("expected a y path track");
    };
    let AnimationKind::Path(width_track) = animations[2].kind() else {
        panic!("expected a width path track");
    };
    let AnimationKind::Path(height_track) = animations[3].kind() else {
        panic!("expected a height path track");
    };

    // When: the four shape geometry tracks are baked.
    let x = x_track.keyframes()[1].path().bounds();
    let y = y_track.keyframes()[1].path().bounds();
    let width = width_track.keyframes()[1].path().bounds();
    let height = height_track.keyframes()[1].path().bounds();

    // Then: x/width use 200, y/height use 100.
    assert_eq!(x.x(), 100.0);
    assert_eq!(y.y(), 50.0);
    assert_eq!(width.width(), 100.0);
    assert_eq!(height.height(), 50.0);

    // Given: image tracks use the same static resolver for physical units.
    let image_tree = Tree::from_str(
        &format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><image href='{PNG}' x='5mm' y='5mm' width='5mm' height='5mm'><animate attributeName='x' from='5mm' to='5mm' dur='1s'/><animate attributeName='y' from='5mm' to='5mm' dur='1s'/><animate attributeName='width' from='5mm' to='5mm' dur='1s'/><animate attributeName='height' from='5mm' to='5mm' dur='1s'/></image></svg>"
        ),
        &Options::default(),
    )
    .unwrap();
    let image_root = group(&image_tree.root().children()[0]);
    let image_animation = image_root.animation().unwrap();
    let (static_x, static_y, static_width, static_height) =
        image_animation.image().unwrap().static_quad();
    let animations = image_animation.animations();
    let AnimationKind::ImageX(x_track) = animations[0].kind() else {
        panic!("expected an image x track");
    };
    let AnimationKind::ImageY(y_track) = animations[1].kind() else {
        panic!("expected an image y track");
    };
    let AnimationKind::ImageWidth(width_track) = animations[2].kind() else {
        panic!("expected an image width track");
    };
    let AnimationKind::ImageHeight(height_track) = animations[3].kind() else {
        panic!("expected an image height track");
    };

    // When: image geometry keyframes are parsed.
    let image_values = [
        *x_track.keyframes()[1].value(),
        *y_track.keyframes()[1].value(),
        *width_track.keyframes()[1].value(),
        *height_track.keyframes()[1].value(),
    ];

    // Then: every physical-unit keyframe equals its static component.
    assert_eq!(
        image_values,
        [static_x, static_y, static_width, static_height]
    );
}

#[test]
fn smil_geometry_delta_units() {
    // Given: two image x animations in the same 200px horizontal coordinate space.
    let tree = Tree::from_str(
        &format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><image href='{PNG}' width='10' height='10'><animate attributeName='x' from='10%' to='50%' dur='1s'/></image><image href='{PNG}' x='50%' width='10' height='10'><animate attributeName='x' by='25%' additive='sum' dur='1s'/></image></svg>"
        ),
        &Options::default(),
    )
    .unwrap();
    let from_to = group(&tree.root().children()[0]).animation().unwrap();
    let by = group(&tree.root().children()[1]).animation().unwrap();
    let AnimationKind::ImageX(from_to_track) = from_to.animations()[0].kind() else {
        panic!("expected a from/to image x track");
    };
    let AnimationKind::ImageX(by_track) = by.animations()[0].kind() else {
        panic!("expected a by image x track");
    };

    // When: the final from/to value and additive delta are sampled from their tracks.
    let from_to_final = *from_to_track.keyframes()[1].value();
    let delta = *by_track.keyframes()[1].value();
    let base = by.image().unwrap().static_quad().0;

    // Then: 50% is 100, and a 25% additive delta ends at 150.
    assert_eq!(from_to_final, 100.0);
    assert_eq!(base + delta, 150.0);
}

#[test]
fn geometry_invalid_units() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();

    // Given: static negative percentage geometry is invalid and removed.
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let static_tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><rect width='-10%' height='10'/></svg>",
        &Options::default(),
    )
    .unwrap();
    assert!(static_tree.root().children().is_empty());

    // When: the equivalent invalid animation value follows a valid percentage.
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let animated_tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><rect width='10' height='10'><animate attributeName='width' values='10%;-10%' dur='1s'/></rect></svg>",
        &Options::default(),
    )
    .unwrap();
    let animation = &path(&animated_tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a geometry path track");
    };

    // Then: the resolved invalid value is dropped and reports the static-space scalar.
    assert_eq!(track.keyframes().len(), 1);
    assert_eq!(track.keyframes()[0].path().bounds().width(), 20.0);
    assert!(
        WARNINGS
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .any(|warning| warning == "Invalid geometry animation value: '-20'.")
    );
}
