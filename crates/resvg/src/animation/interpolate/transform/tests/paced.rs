use tiny_skia::Transform;
use usvg::TransformFunction;

use super::common::{approx, approx_transform, paced, sample, smil_track, PACED_WARNING};

#[test]
fn x4_varying_rotate_center_falls_back_to_linear() {
    // Given: a paced rotate with a changing center.
    let track = smil_track("rotate", "0 0 0;90 10 0");

    // When: it is sampled at the midpoint.
    let matrix = sample(&track, &paced(), 0.5);

    // Then: fallback stays linear, not discrete, with the required warning text.
    approx(matrix.sx, 0.7071068);
    approx(matrix.ky, 0.7071068);
    approx(matrix.kx, -0.7071068);
    approx(matrix.sy, 0.7071068);
    approx(matrix.tx, 1.4644661);
    approx(matrix.ty, -3.535534);
    assert_eq!(
        PACED_WARNING,
        "Paced interpolation is not supported here; using linear."
    );
}

#[test]
fn x4b_near_equal_rotate_centers_use_epsilon_pacing() {
    // Given: centers differing by less than f32::EPSILON.
    let track = smil_track("rotate", "0 0 0;10 0.00000005 0;110 0 0");

    // When: paced interpolation samples the middle normalized progress.
    let matrix = sample(&track, &paced(), 0.5);

    // Then: angle-distance pacing reaches 55 degrees rather than linear 10.
    approx(matrix.ky, 55_f32.to_radians().sin());
}

#[test]
fn x5_constant_rotate_center_uses_angle_distance() {
    // Given: constant-center rotate segments whose angle distances are 90 and 270.
    let track = smil_track("rotate", "0 2 3;90 2 3;360 2 3");

    // When: a quarter of the total paced distance is sampled.
    let matrix = sample(&track, &paced(), 0.25);

    // Then: it lands at the first 90-degree endpoint.
    approx_transform(matrix, Transform::from_rotate_at(90.0, 2.0, 3.0));
}

#[test]
fn x6_single_function_signatures_keep_their_paced_metrics() {
    // Given: translate, scale, and skew tracks with known segment distances.
    let translate = smil_track("translate", "0 0;3 4;3 12");
    let scale = smil_track("scale", "1;4 5;4 11");
    let skew_x = smil_track("skewX", "0;20;100");
    let skew_y = smil_track("skewY", "0;20;100");

    // When: each track is sampled at the first paced segment boundary.
    let translated = sample(&translate, &paced(), 5.0 / 13.0);
    let scaled = sample(&scale, &paced(), 5.0 / 11.0);
    let skewed_x = sample(&skew_x, &paced(), 0.2);
    let skewed_y = sample(&skew_y, &paced(), 0.2);

    // Then: every supported signature retains its historical metric.
    approx_transform(translated, Transform::from_translate(3.0, 4.0));
    approx_transform(scaled, Transform::from_scale(4.0, 5.0));
    assert!(skewed_x.kx > 0.0);
    assert!(skewed_y.ky > 0.0);
    assert!(matches!(
        skew_x.keyframes()[0].value().as_slice(),
        [TransformFunction::SkewX(_)]
    ));
}
