use tiny_skia::Transform;
use usvg::TransformFunction;

use super::common::{approx, approx_transform, linear, sample, smil_track};

#[test]
fn x1_translate_lowering_bakes_omitted_y() {
    // Given: a SMIL translate with one omitted y component.
    let track = smil_track("translate", "10;30 20");

    // When: the canonical list is sampled halfway.
    let matrix = sample(&track, &linear(), 0.5);

    // Then: every keyframe has an explicit two-axis translate.
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [TransformFunction::Translate(x, y)] if *x == 10.0 && *y == 0.0
    ));
    approx_transform(matrix, Transform::from_translate(20.0, 10.0));
}

#[test]
fn x2_scale_lowering_bakes_each_missing_y_from_its_x() {
    // Given: scale keyframes with mixed one- and two-component syntax.
    let track = smil_track("scale", "2;3 5");

    // When: the canonical list is sampled halfway.
    let matrix = sample(&track, &linear(), 0.5);

    // Then: omitted y is that keyframe's x, not a track-global default.
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [TransformFunction::Scale(x, y)] if *x == 2.0 && *y == 2.0
    ));
    approx_transform(matrix, Transform::from_scale(2.5, 3.5));
}

#[test]
fn x3_rotate_lowering_preserves_parameter_lerp() {
    // Given: a centered rotate whose angle crosses 180 degrees.
    let track = smil_track("rotate", "0 2 -3;270 10 5");

    // When: the lowered list is sampled across its range.
    let midpoint = sample(&track, &linear(), 0.5);

    // Then: centered rotate becomes a uniform translate/rotate/translate list.
    assert!(
        track
            .keyframes()
            .iter()
            .all(|keyframe| keyframe.value().len() == 3)
    );
    approx_transform(midpoint, Transform::from_rotate_at(135.0, 6.0, 1.0));
    assert!(midpoint.ky > 0.0);
}

#[test]
fn x3b_mixed_rotate_arity_stays_smooth() {
    // Given: a bare rotate followed by a centered rotate.
    let track = smil_track("rotate", "0;90 10 0");

    // When: the canonical function list is sampled halfway.
    let midpoint = sample(&track, &linear(), 0.5);

    // Then: uniform three-element lowering avoids a discrete identity step.
    approx_transform(midpoint, Transform::from_rotate_at(45.0, 5.0, 0.0));
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [
            TransformFunction::Translate(_, _),
            TransformFunction::Rotate(_),
            TransformFunction::Translate(_, _),
        ]
    ));
}

#[test]
fn x10_lowered_terminal_matrix_keeps_accumulation_product() {
    // Given: a lowered SMIL translate track with a non-identity terminal value.
    let track = smil_track("translate", "0 0;10 20");

    // When: the current and terminal matrices are composed twice.
    let mut accumulated = sample(&track, &linear(), 0.5);
    let terminal = sample(&track, &linear(), 1.0);
    for _ in 0..2 {
        accumulated = accumulated.pre_concat(terminal);
    }

    // Then: the function-list terminal matrix has the established sum product.
    approx(accumulated.tx, 25.0);
    approx(accumulated.ty, 50.0);
}
