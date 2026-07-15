use tiny_skia::Transform;
use usvg::{AnimationKind, Options, TransformFunction, Tree};

use super::common::{approx_transform, linear, sample};

#[test]
fn x11_gradient_transform_has_no_css_origin_wrapper() {
    // Given: a SMIL gradientTransform rotation with default centers.
    let tree = Tree::from_str(
        "<svg xmlns='http://www.w3.org/2000/svg'><defs><linearGradient id='g'><animateTransform attributeName='gradientTransform' type='rotate' values='0;90' dur='1s'/><stop offset='0' stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/></svg>",
        &Options::default(),
    )
    .unwrap_or_else(|error| panic!("gradient transform parse failed: {error:?}"));
    let animation = &tree.linear_gradients()[0].animation().unwrap().animations()[0];
    let AnimationKind::GradientTransform(track) = animation.kind() else {
        panic!(
            "expected a gradient transform track, got {:?}",
            animation.kind()
        );
    };

    // When: its canonical function list is sampled halfway.
    let matrix = sample(track, &linear(), 0.5);

    // Then: no CSS origin wrapper appears on a gradientTransform track.
    assert!(
        track
            .keyframes()
            .iter()
            .all(|keyframe| matches!(keyframe.value().as_slice(), [TransformFunction::Rotate(_)]))
    );
    approx_transform(matrix, Transform::from_rotate(45.0));
}
