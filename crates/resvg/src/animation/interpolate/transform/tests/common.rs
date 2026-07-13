use tiny_skia::Transform;
use usvg::{
    AnimationKind, CalcMode, Easing, Keyframe, Node, NormalizedF32, Options, Track,
    TransformFunction, Tree,
};

pub(super) use super::super::super::locate::PACED_WARNING;

pub(super) fn n(value: f32) -> NormalizedF32 {
    NormalizedF32::new_clamped(value)
}

pub(super) fn linear() -> Easing {
    Easing::new(CalcMode::Linear, None, None)
}

pub(super) fn paced() -> Easing {
    Easing::new(CalcMode::Paced, None, None)
}

pub(super) fn track(values: &[(f32, Vec<TransformFunction>)]) -> Track<Vec<TransformFunction>> {
    Track::new(
        values
            .iter()
            .map(|(offset, value)| Keyframe::new(n(*offset), value.clone(), None))
            .collect(),
    )
}

pub(super) fn sample(
    track: &Track<Vec<TransformFunction>>,
    easing: &Easing,
    progress: f32,
) -> Transform {
    super::super::sample_transform(track, easing, None, progress)
        .unwrap_or_else(|| panic!("transform track unexpectedly had no sample"))
}

pub(super) fn smil_track(type_: &str, values: &str) -> Track<Vec<TransformFunction>> {
    let tree = Tree::from_str(
        &format!(
            "<svg xmlns='http://www.w3.org/2000/svg'><g><animateTransform attributeName='transform' type='{type_}' values='{values}' dur='1s'/><rect width='4' height='4'/></g></svg>"
        ),
        &Options::default(),
    )
    .unwrap_or_else(|error| panic!("SMIL transform parse failed: {error:?}"));
    let Node::Group(group) = &tree.root().children()[0] else {
        panic!("expected an animated group");
    };
    let AnimationKind::Transform(track) = group.animations()[0].kind() else {
        panic!("expected a transform track");
    };
    track.clone()
}

pub(super) fn approx(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 1e-4,
        "expected {expected}, got {actual}"
    );
}

pub(super) fn approx_transform(actual: Transform, expected: Transform) {
    approx(actual.sx, expected.sx);
    approx(actual.ky, expected.ky);
    approx(actual.kx, expected.kx);
    approx(actual.sy, expected.sy);
    approx(actual.tx, expected.tx);
    approx(actual.ty, expected.ty);
}
