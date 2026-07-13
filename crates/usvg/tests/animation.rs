// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(feature = "animation")]

use std::sync::{Mutex, Once, OnceLock};

use usvg::{
    Additive, AnimationKind, AnimationSource, CalcMode, Direction, Node, Options, StepPosition,
    TimingFunction, TransformFunction, Tree,
};

#[path = "animation/geometry_units.rs"]
mod geometry_units;

const NS: &str = "http://www.w3.org/2000/svg";
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9JTxAAAAAASUVORK5CYII=";

static WARNINGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

struct CaptureLogger;

impl log::Log for CaptureLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if let Some(warnings) = WARNINGS.get() {
            warnings.lock().unwrap().push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

static LOGGER_INIT: Once = Once::new();
static WARN_GUARD: Mutex<()> = Mutex::new(());

fn init_capture() {
    LOGGER_INIT.call_once(|| {
        WARNINGS.get_or_init(|| Mutex::new(Vec::new()));
        log::set_logger(&CaptureLogger).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
}

fn parse(body: &str) -> Tree {
    Tree::from_str(
        &format!("<svg xmlns='{NS}' width='20' height='20'>{body}</svg>"),
        &Options::default(),
    )
    .unwrap()
}

fn group(node: &Node) -> &usvg::Group {
    match node {
        Node::Group(group) => group,
        _ => panic!("expected group"),
    }
}

fn path(node: &Node) -> &usvg::Path {
    match node {
        Node::Path(path) => path,
        _ => panic!("expected path"),
    }
}

#[test]
fn animated_group_survives_pruning() {
    let tree = parse(
        "<g><animateTransform attributeName='transform' type='translate' from='0 0' to='2 3' dur='1s'/><rect width='4' height='4'/></g>",
    );
    let group = group(&tree.root().children()[0]);
    assert!(matches!(
        group.animations()[0].kind(),
        AnimationKind::Transform(_)
    ));
}

#[test]
fn smil_mixed_rotate_arity_lowers_to_uniform_function_lists() {
    // Given: a bare rotation followed by one with an explicit center.
    let tree = parse(
        "<g><animateTransform attributeName='transform' type='rotate' values='0;90 10 0' dur='1s'/><rect width='4' height='4'/></g>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Transform(track) = animation.kind() else {
        panic!("expected a transform track");
    };

    // When: the parser lowers the SMIL values into the public tree model.
    let keyframes = track.keyframes();

    // Then: every keyframe carries the centered three-function signature.
    assert!(matches!(
        keyframes[0].value().as_slice(),
        [
            TransformFunction::Translate(x0, y0),
            TransformFunction::Rotate(angle),
            TransformFunction::Translate(x1, y1),
        ] if *x0 == 0.0 && *y0 == 0.0 && *angle == 0.0 && *x1 == 0.0 && *y1 == 0.0
    ));
    assert!(matches!(
        keyframes[1].value().as_slice(),
        [
            TransformFunction::Translate(x0, y0),
            TransformFunction::Rotate(angle),
            TransformFunction::Translate(x1, y1),
        ] if *x0 == 10.0 && *y0 == 0.0 && *angle == 90.0 && *x1 == -10.0 && *y1 == 0.0
    ));
}

#[test]
fn shape_transform_and_opacity_use_wrapper_groups() {
    let transform = parse(
        "<rect width='4' height='4'><animateTransform attributeName='transform' type='translate' from='0 0' to='2 3' dur='1s'/></rect>",
    );
    let transform_group = group(&transform.root().children()[0]);
    assert!(matches!(
        transform_group.animations()[0].kind(),
        AnimationKind::Transform(_)
    ));
    assert!(matches!(transform_group.children()[0], Node::Path(_)));

    let opacity = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s'/></rect>",
    );
    let opacity_group = group(&opacity.root().children()[0]);
    assert!(matches!(
        opacity_group.animations()[0].kind(),
        AnimationKind::Opacity(_)
    ));
    assert!(matches!(opacity_group.children()[0], Node::Path(_)));
}

#[test]
fn image_geometry_tracks_attach_to_image_root() {
    let tree = parse(&format!(
        "<image href='{PNG}' y='1' width='4' height='4'><animate attributeName='y' from='1' to='3' dur='2s'/></image>"
    ));
    let root = group(&tree.root().children()[0]);
    assert!(matches!(
        root.animations()[0].kind(),
        AnimationKind::ImageY(_)
    ));
    assert!(matches!(root.children()[0], Node::Image(_)));
}

#[test]
fn concurrent_image_geometry_tracks_retain_timing() {
    let tree = parse(&format!(
        "<image href='{PNG}' width='4' height='4'><animate attributeName='width' from='4' to='8' dur='2s'/><animate attributeName='height' from='4' to='9' begin='1s' dur='4s'/></image>"
    ));
    let animation = group(&tree.root().children()[0]).animation().unwrap();
    assert_eq!(animation.animations().len(), 2);
    assert!(matches!(
        animation.animations()[0].kind(),
        AnimationKind::ImageWidth(_)
    ));
    assert!(matches!(
        animation.animations()[1].kind(),
        AnimationKind::ImageHeight(_)
    ));
    assert_eq!(
        animation.animations()[0].timing().iteration_dur(),
        Some(2.0)
    );
    assert_eq!(
        animation.animations()[1].timing().iteration_dur(),
        Some(4.0)
    );
}

#[test]
fn mask_content_animation_is_attached() {
    let tree = parse(
        "<defs><mask id='mask'><rect width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s'/></rect></mask></defs><rect width='4' height='4' mask='url(#mask)'/>",
    );
    let mask = group(&tree.root().children()[0]).mask().unwrap();
    let animated = group(&mask.root().children()[0]);
    assert!(matches!(
        animated.animations()[0].kind(),
        AnimationKind::Opacity(_)
    ));
}

#[test]
fn display_none_reveal_nodes_are_retained_in_tree_and_clip_path() {
    let tree = parse(
        "<defs><clipPath id='clip'><rect display='none' width='4' height='4'><set attributeName='display' to='inline' begin='1s'/></rect></clipPath></defs><rect display='none' width='4' height='4'><set attributeName='display' to='inline' begin='1s'/></rect><rect width='4' height='4' clip-path='url(#clip)'/>",
    );
    let main = path(&tree.root().children()[0]).animation().unwrap();
    assert!(main.base_hidden());
    let clip = group(&tree.root().children()[1]).clip_path().unwrap();
    let clipped = path(&clip.root().children()[0]).animation().unwrap();
    assert!(clipped.base_hidden());
}

#[test]
fn paint_and_stroke_carriers_preserve_disabled_static_state() {
    let fill = parse(
        "<path d='M0 0 L4 4' fill='none'><animate attributeName='fill' from='red' to='blue' dur='1s'/></path>",
    );
    let fill_carrier = path(&fill.root().children()[0])
        .animation()
        .unwrap()
        .fill()
        .unwrap();
    assert!(fill_carrier.underlying_disabled());

    let stroke = parse(
        "<path d='M0 0 L4 4' stroke='red' stroke-width='0'><animate attributeName='stroke-width' from='0' to='10' dur='1s'/></path>",
    );
    let stroke_carrier = path(&stroke.root().children()[0])
        .animation()
        .unwrap()
        .stroke()
        .unwrap();
    assert!(stroke_carrier.paint().is_some());
    assert_eq!(stroke_carrier.width(), 0.0);
}

#[test]
fn zero_static_geometry_uses_a_path_animation_carrier() {
    let tree = parse(
        "<rect width='0' height='4'><animate attributeName='width' from='0' to='8' dur='1s'/></rect>",
    );
    let animation = path(&tree.root().children()[0]).animation().unwrap();
    assert!(!animation.path().unwrap().underlying_renderable());
    assert!(matches!(
        animation.animations()[0].kind(),
        AnimationKind::Path(_)
    ));
}

#[test]
fn text_animation_warns_without_attachment() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let tree = parse(
        "<text x='0' y='10'>text<animate attributeName='opacity' from='0' to='1' dur='1s'/></text><rect width='4' height='4'/>",
    );
    assert!(matches!(tree.root().children()[0], Node::Path(_)));
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "Animation of text elements is not supported."));
}

#[test]
fn remote_text_animation_warns_without_attachment() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let _tree = parse(
        "<animate xlink:href='#text' attributeName='x' to='10' dur='1s' xmlns:xlink='http://www.w3.org/1999/xlink'/><rect width='4' height='4'/>",
    );
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "Animation of text elements is not supported."));
}

#[test]
fn gradient_shared_by_two_shapes_keeps_tracks() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'><animate attributeName='stop-color' from='red' to='blue' dur='1s'/></stop><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/><rect width='4' height='4' fill='url(#g)'/>",
    );
    assert_eq!(tree.linear_gradients().len(), 2);
    assert!(tree
        .linear_gradients()
        .iter()
        .all(|gradient| gradient.animation().is_some()));
}

#[test]
fn gradient_stop_track_is_readable() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'><animate attributeName='stop-color' from='red' to='blue' dur='1s'/></stop><stop offset='1' stop-color='green'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/>",
    );
    let gradient = &tree.linear_gradients()[0];
    let animation = gradient.animation().unwrap();
    assert_eq!(animation.source_stops().len(), 2);
    assert!(matches!(
        animation.source_stops()[0].animations()[0].kind(),
        AnimationKind::StopColor(_)
    ));
    assert!(animation.source_stops()[1].animations().is_empty());
    assert_eq!(animation.source_index_of(0), Some(0));
    assert_eq!(animation.source_index_of(1), Some(1));
}

#[test]
fn three_stop_shared_offset_middle_is_kept() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0.5' stop-color='red'/><stop offset='0.5' stop-color='green'><animate attributeName='offset' from='0.5' to='0.9' dur='1s'/></stop><stop offset='0.5' stop-color='blue'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/>",
    );
    let gradient = &tree.linear_gradients()[0];
    assert_eq!(gradient.stops().len(), 3);
    let animation = gradient.animation().unwrap();
    assert_eq!(animation.source_stops().len(), 3);
    assert!(matches!(
        animation.source_stops()[1].animations()[0].kind(),
        AnimationKind::StopOffset(_)
    ));
}

#[test]
fn one_stop_animation_is_not_collapsed() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'><animate attributeName='stop-color' from='red' to='blue' dur='1s'/></stop></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/>",
    );
    assert_eq!(tree.linear_gradients().len(), 1);
    let gradient = &tree.linear_gradients()[0];
    assert_eq!(gradient.stops().len(), 2);
    let animation = gradient.animation().unwrap();
    assert_eq!(animation.source_stops().len(), 1);
    assert!(matches!(
        animation.source_stops()[0].animations()[0].kind(),
        AnimationKind::StopColor(_)
    ));
}

#[test]
fn objectboundingbox_geometry_keyframes_stay_native() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'/><stop offset='1' stop-color='blue'/><animate attributeName='x1' from='0.2' to='0.8' dur='1s'/></linearGradient></defs><rect width='10' height='10' fill='url(#g)'/>",
    );
    let gradient = &tree.linear_gradients()[0];
    let transform = gradient.transform();
    assert_eq!(transform.sx, 10.0);
    assert_eq!(transform.sy, 10.0);
    let animation = gradient.animation().unwrap();
    let track = match animation.animations()[0].kind() {
        AnimationKind::GradientGeometry(geometry) => geometry.track(),
        other => panic!("expected gradient geometry, got {other:?}"),
    };
    assert_eq!(*track.keyframes()[0].value(), 0.2);
    assert_eq!(*track.keyframes()[1].value(), 0.8);
}

#[test]
fn clone_preservation_across_different_bboxes() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'/><stop offset='1' stop-color='blue'/><animate attributeName='x1' from='0' to='1' dur='1s'/></linearGradient></defs><rect width='10' height='10' fill='url(#g)'/><rect width='4' height='8' fill='url(#g)'/>",
    );
    assert_eq!(tree.linear_gradients().len(), 2);
    assert!(tree
        .linear_gradients()
        .iter()
        .all(|gradient| gradient.animation().is_some()));
}

#[test]
fn view_box_animation_to_transform_is_public() {
    let tree = parse("<animate attributeName='viewBox' from='0 0 20 20' to='0 0 40 40' dur='1s'/>");
    let animation = tree.view_box_animation().unwrap();
    let sampled = usvg::NonZeroRect::from_xywh(0.0, 0.0, 40.0, 40.0).unwrap();
    let transform = animation.to_transform(sampled, tree.size());
    assert_eq!(transform.sx, 0.5);
    assert_eq!(transform.sy, 0.5);
}

#[test]
fn view_box_narrowing_keeps_first_warns_second() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let tree = parse(
        "<animate attributeName='viewBox' from='0 0 20 20' to='0 0 10 10' dur='1s'/><animate attributeName='viewBox' from='0 0 20 20' to='0 0 40 40' dur='1s'/>",
    );
    let animation = tree.view_box_animation().unwrap();
    assert_eq!(animation.track().keyframes()[1].value().width(), 10.0);
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "Only a single non-additive viewBox animation is supported."));
}

#[test]
fn multi_interval_begins_yield_two_intervals() {
    let tree = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' begin='0s;3s' dur='1s' from='0' to='1'/></rect>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let timing = animation.timing();
    assert_eq!(timing.intervals().len(), 2);
    assert_eq!(timing.intervals()[0].interval().begin(), 0.0);
    assert_eq!(timing.intervals()[1].interval().begin(), 3.0);
}

#[test]
fn values_with_key_times_place_keyframes_at_offsets() {
    let tree = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' values='0;0.5;1' keyTimes='0;0.3;1' dur='1s'/></rect>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Opacity(track) = animation.kind() else {
        panic!("expected an opacity track");
    };
    assert_eq!(track.keyframes().len(), 3);
    assert_eq!(track.keyframes()[0].offset().get(), 0.0);
    assert!((track.keyframes()[1].offset().get() - 0.3).abs() < 1e-6);
    assert_eq!(track.keyframes()[2].offset().get(), 1.0);
    assert_eq!(track.keyframes()[1].value().get(), 0.5);
}

#[test]
fn key_splines_are_parsed_into_easing() {
    let tree = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' values='0;1' calcMode='spline' keySplines='0.5 0 0.5 1' dur='1s'/></rect>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    assert!(matches!(animation.easing().calc_mode(), CalcMode::Spline));
    let splines = animation.easing().key_splines().unwrap();
    assert_eq!(splines.len(), 1);
    assert_eq!(splines[0], [0.5, 0.0, 0.5, 1.0]);
}

#[test]
fn set_on_path_data_bakes_a_discrete_path_track() {
    let tree = parse("<path d='M0 0 L4 4'><set attributeName='d' to='M8 8 L12 12'/></path>");
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a path track");
    };
    assert_eq!(track.keyframes().len(), 1);
    assert!(matches!(animation.easing().calc_mode(), CalcMode::Discrete));
}

#[test]
fn set_produces_a_single_keyframe_discrete_track() {
    let tree =
        parse("<rect width='4' height='4'><set attributeName='opacity' to='0.5' dur='1s'/></rect>");
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Opacity(track) = animation.kind() else {
        panic!("expected an opacity track");
    };
    assert_eq!(track.keyframes().len(), 1);
    assert_eq!(track.keyframes()[0].value().get(), 0.5);
    assert!(matches!(animation.easing().calc_mode(), CalcMode::Discrete));
}

#[test]
fn paced_rotate_transform_keeps_paced_easing() {
    let tree = parse(
        "<g><rect width='10' height='4'/><animateTransform attributeName='transform' type='rotate' values='0 240 180;30 240 180;300 240 180;360 240 180' calcMode='paced' dur='4s' fill='freeze'/></g>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    assert!(matches!(animation.easing().calc_mode(), CalcMode::Paced));
}

#[test]
fn discrete_geometry_from_to_switches_at_half_duration() {
    let tree = parse(
        "<rect width='50' height='20'><animate attributeName='height' calcMode='discrete' from='200' to='20' dur='4s'/></rect>",
    );
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a path track");
    };
    assert_eq!(track.keyframes()[1].offset().get(), 0.5);
}

#[test]
fn bare_by_opacity_is_a_sum_delta_track() {
    let tree = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' by='0.5' dur='1s'/></rect>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    assert!(matches!(animation.additive(), Additive::Sum));
    let AnimationKind::Opacity(track) = animation.kind() else {
        panic!("expected an opacity track");
    };
    assert_eq!(track.keyframes()[0].value().get(), 0.0);
    assert_eq!(track.keyframes()[1].value().get(), 0.5);
}

#[test]
fn additive_transform_pair_is_preserved() {
    let tree = parse(
        "<rect width='4' height='4'><animateTransform attributeName='transform' type='translate' from='0 0' to='2 3' dur='1s'/><animateTransform attributeName='transform' type='scale' additive='sum' from='1' to='2' dur='1s'/></rect>",
    );
    let wrapper = group(&tree.root().children()[0]);
    assert_eq!(wrapper.animations().len(), 2);
    assert!(matches!(
        wrapper.animations()[0].kind(),
        AnimationKind::Transform(_)
    ));
    assert!(matches!(
        wrapper.animations()[1].kind(),
        AnimationKind::Transform(_)
    ));
    assert!(matches!(
        wrapper.animations()[0].additive(),
        Additive::Replace
    ));
    assert!(matches!(wrapper.animations()[1].additive(), Additive::Sum));
}

#[test]
fn gradient_stop_color_track_offsets_are_readable() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'><animate attributeName='stop-color' from='red' to='blue' dur='1s'/></stop><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/>",
    );
    let gradient = &tree.linear_gradients()[0];
    let animation = &gradient.animation().unwrap().source_stops()[0].animations()[0];
    let AnimationKind::StopColor(track) = animation.kind() else {
        panic!("expected a stop-color track");
    };
    assert_eq!(track.keyframes().len(), 2);
    assert_eq!(track.keyframes()[0].offset().get(), 0.0);
    assert_eq!(track.keyframes()[1].offset().get(), 1.0);
}

#[test]
fn radial_gradient_geometry_track_stays_native() {
    let tree = parse(
        "<defs><radialGradient id='g'><stop offset='0' stop-color='red'/><stop offset='1' stop-color='blue'/><animate attributeName='r' from='0.2' to='0.8' dur='1s'/></radialGradient></defs><rect width='10' height='10' fill='url(#g)'/>",
    );
    let gradient = &tree.radial_gradients()[0];
    let animation = &gradient.animation().unwrap().animations()[0];
    let AnimationKind::GradientGeometry(geometry) = animation.kind() else {
        panic!("expected a gradient geometry track");
    };
    assert_eq!(*geometry.track().keyframes()[0].value(), 0.2);
    assert_eq!(*geometry.track().keyframes()[1].value(), 0.8);
}

#[test]
fn gradient_transform_animation_maps_to_gradient_transform_kind() {
    let tree = parse(
        "<defs><linearGradient id='g'><stop offset='0' stop-color='red'/><stop offset='1' stop-color='blue'/><animateTransform attributeName='gradientTransform' type='translate' from='0 0' to='2 3' dur='1s'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/>",
    );
    let gradient = &tree.linear_gradients()[0];
    let animation = &gradient.animation().unwrap().animations()[0];
    assert!(matches!(
        animation.kind(),
        AnimationKind::GradientTransform(_)
    ));
}

#[test]
fn stroke_dasharray_track_is_readable() {
    let tree = parse(
        "<path d='M0 0 L4 4' stroke='black' stroke-dasharray='5,5'><animate attributeName='stroke-dasharray' values='5,5;10,10' dur='1s'/></path>",
    );
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::StrokeDasharray(track) = animation.kind() else {
        panic!("expected a stroke-dasharray track");
    };
    assert_eq!(track.keyframes()[0].value().as_slice(), [5.0, 5.0]);
    assert_eq!(track.keyframes()[1].value().as_slice(), [10.0, 10.0]);
}

#[test]
fn polygon_points_animation_bakes_a_path_track() {
    let tree = parse(
        "<polygon points='0,0 4,0 4,4'><animate attributeName='points' values='0,0 4,0 4,4;0,0 8,0 8,8' dur='1s'/></polygon>",
    );
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::Path(track) = animation.kind() else {
        panic!("expected a baked path track");
    };
    assert_eq!(track.keyframes().len(), 2);
    assert!(track.keyframes()[0].renderable());
}

#[test]
fn view_box_track_keyframes_are_readable() {
    let tree = parse("<animate attributeName='viewBox' from='0 0 20 20' to='0 0 40 40' dur='1s'/>");
    let animation = tree.view_box_animation().unwrap();
    assert_eq!(animation.track().keyframes().len(), 2);
    assert_eq!(animation.track().keyframes()[0].value().width(), 20.0);
    assert_eq!(animation.track().keyframes()[1].value().width(), 40.0);
}

#[test]
fn display_reveal_marks_base_hidden_and_records_a_track() {
    let tree = parse(
        "<rect display='none' width='4' height='4'><set attributeName='display' to='inline' begin='1s'/></rect><rect width='4' height='4'/>",
    );
    let animation = path(&tree.root().children()[0]).animation().unwrap();
    assert!(animation.base_hidden());
    assert!(matches!(
        animation.animations()[0].kind(),
        AnimationKind::Display(_)
    ));
}

#[test]
fn motion_key_points_are_recorded() {
    let tree = parse(
        "<rect width='4' height='4'><animateMotion path='M0 0 L10 10' keyPoints='0;0.5;1' keyTimes='0;0.5;1' dur='1s'/></rect>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Motion(track) = animation.kind() else {
        panic!("expected a motion track");
    };
    assert_eq!(track.key_points().unwrap().len(), 3);
    assert_eq!(track.key_points().unwrap()[1].get(), 0.5);
}

#[test]
fn freeze_and_remove_fill_modes_are_distinguished() {
    let freeze = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s' fill='freeze'/></rect>",
    );
    let freeze_animation = &group(&freeze.root().children()[0]).animations()[0];
    assert_eq!(freeze_animation.timing().intervals()[0].held(), Some(1.0));

    let remove = parse(
        "<rect width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s' fill='remove'/></rect>",
    );
    let remove_animation = &group(&remove.root().children()[0]).animations()[0];
    assert_eq!(remove_animation.timing().intervals()[0].held(), None);
}

#[test]
fn css_steps_timing_function_is_parsed() {
    let tree = parse(
        "<style>@keyframes move { from { transform: translate(0px,0px); } to { transform: translate(4px,0px); } } #box { animation: move 4s steps(4, jump-end); }</style><rect id='box' width='4' height='4'/>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    assert!(matches!(animation.source(), AnimationSource::Css));
    assert!(matches!(
        animation.easing().timing_function(),
        Some(TimingFunction::Steps(4, StepPosition::JumpEnd))
    ));
}

#[test]
fn css_negative_delay_is_preserved() {
    let tree = parse(
        "<style>@keyframes move { from { transform: translate(0px,0px); } to { transform: translate(4px,0px); } } #box { animation: move 4s linear -1s; }</style><rect id='box' width='4' height='4'/>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let interval = animation.timing().intervals()[0].interval();
    assert_eq!(interval.begin(), -1.0);
    assert_eq!(animation.timing().iteration_dur(), Some(4.0));
}

#[test]
fn css_alternate_reverse_direction_is_parsed() {
    let tree = parse(
        "<style>@keyframes fade { from { opacity: 1; } to { opacity: 0; } } #box { animation: fade 4s linear alternate-reverse; }</style><rect id='box' width='4' height='4'/>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    assert!(matches!(
        animation.timing().direction(),
        Direction::AlternateReverse
    ));
}

#[test]
fn css_two_animations_attach_to_one_node() {
    let tree = parse(
        "<style>@keyframes move { from { transform: translate(0px,0px); } to { transform: translate(4px,4px); } } @keyframes fade { from { opacity: 1; } to { opacity: 0.2; } } #box { animation: move 4s linear, fade 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    let wrapper = group(&tree.root().children()[0]);
    assert_eq!(wrapper.animations().len(), 2);
    assert!(matches!(
        wrapper.animations()[0].kind(),
        AnimationKind::Transform(_)
    ));
    assert!(matches!(
        wrapper.animations()[1].kind(),
        AnimationKind::Opacity(_)
    ));
}

#[test]
fn css_percent_keyframe_offset_is_placed() {
    let tree = parse(
        "<style>@keyframes pulse { 0% { opacity: 1; } 50% { opacity: 0.5; } 100% { opacity: 1; } } #box { animation: pulse 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Opacity(track) = animation.kind() else {
        panic!("expected an opacity track");
    };
    assert_eq!(track.keyframes().len(), 3);
    assert_eq!(track.keyframes()[1].offset().get(), 0.5);
    assert_eq!(track.keyframes()[1].value().get(), 0.5);
}

#[test]
fn css_transform_origin_percent_values_are_baked() {
    let tree = parse(
        "<style>@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(90deg); } } #box { transform-origin: 25% 75%; animation: spin 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Transform(track) = animation.kind() else {
        panic!("expected a CSS transform track");
    };
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [
            TransformFunction::Translate(x0, y0),
            TransformFunction::Rotate(angle),
            TransformFunction::Translate(x1, y1),
        ] if *x0 == 1.0 && *y0 == 3.0 && *angle == 0.0 && *x1 == -1.0 && *y1 == -3.0
    ));
}

#[test]
fn css_transform_origin_uses_stroke_box_when_requested() {
    let tree = parse(
        "<style>@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(90deg); } } #box { transform-origin: 100% 50%; transform-box: stroke-box; animation: spin 4s linear; }</style><rect id='box' width='10' height='10' stroke='black' stroke-width='10'/>",
    );
    let animation = &group(&tree.root().children()[0]).animations()[0];
    let AnimationKind::Transform(track) = animation.kind() else {
        panic!("expected a CSS transform track");
    };
    assert!(matches!(
        track.keyframes()[0].value().as_slice(),
        [
            TransformFunction::Translate(x0, y0),
            TransformFunction::Rotate(angle),
            TransformFunction::Translate(x1, y1),
        ] if *x0 == 15.0 && *y0 == 5.0 && *angle == 0.0 && *x1 == -15.0 && *y1 == -5.0
    ));
}

#[test]
fn css_stroke_width_is_a_renderable_track() {
    let tree = parse(
        "<style>@keyframes grow { from { stroke-width: 1; } to { stroke-width: 8; } } #line { animation: grow 4s linear; }</style><path id='line' d='M0 0 L4 4' stroke='black' stroke-width='1'/>",
    );
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    let AnimationKind::StrokeWidth(track) = animation.kind() else {
        panic!("expected a stroke-width track");
    };
    assert_eq!(track.keyframes().len(), 2);
    assert_eq!(*track.keyframes()[0].value(), 1.0);
    assert_eq!(*track.keyframes()[1].value(), 8.0);
}

#[test]
fn css_fill_animation_is_suppressed_by_important() {
    let tree = parse(
        "<style>@keyframes recolor { from { fill: red; } to { fill: blue; } } #box { fill: green !important; animation: recolor 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    let animation = &path(&tree.root().children()[0])
        .animation()
        .unwrap()
        .animations()[0];
    assert!(matches!(animation.kind(), AnimationKind::Fill(_)));
    assert!(animation.suppressed_by_important());
}

#[test]
fn css_stop_color_and_stop_opacity_attach_to_a_stop() {
    let tree = parse(
        "<style>@keyframes shift { from { stop-color: #ff0000; stop-opacity: 1; } to { stop-color: #0000ff; stop-opacity: 0.2; } } #s0 { animation: shift 4s linear; }</style><defs><linearGradient id='g'><stop id='s0' offset='0' stop-color='red' stop-opacity='1'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='4' height='4' fill='url(#g)'/>",
    );
    let gradient = &tree.linear_gradients()[0];
    let stop = &gradient.animation().unwrap().source_stops()[0];
    assert_eq!(stop.animations().len(), 2);
    assert!(matches!(
        stop.animations()[0].kind(),
        AnimationKind::StopColor(_)
    ));
    assert!(matches!(
        stop.animations()[1].kind(),
        AnimationKind::StopOpacity(_)
    ));
}

#[test]
fn css_unknown_keyframes_name_warns() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let _tree = parse(
        "<style>#box { animation: missing 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "Unknown keyframes name: 'missing'."));
}

#[test]
fn css_unsupported_property_warns() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let _tree = parse(
        "<style>@keyframes shift { from { color: red; } to { color: blue; } } #box { animation: shift 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "Unsupported CSS property in keyframes: 'color'."));
}

#[test]
fn css_variables_are_not_supported_warns() {
    let _guard = WARN_GUARD.lock().unwrap();
    init_capture();
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let _tree = parse(
        "<style>@keyframes shift { from { fill: var(--a); } to { fill: var(--b); } } #box { animation: shift 4s linear; }</style><rect id='box' width='4' height='4'/>",
    );
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "CSS variables are not supported."));
}

#[test]
fn static_tree_has_no_animations() {
    let tree = parse("<rect width='10' height='10' fill='green'/>");
    assert!(!tree.has_animations());
    assert_eq!(tree.animation_duration(), None);
}

#[test]
fn animated_node_reports_animations() {
    let tree = parse(
        "<rect width='10' height='10'><animate attributeName='opacity' from='0' to='1' dur='3s'/></rect>",
    );
    assert!(tree.has_animations());
    assert_eq!(tree.animation_duration(), Some(3.0));
}

#[test]
fn animation_duration_takes_the_longest_loop() {
    let tree = parse(
        "<rect width='10' height='10'>\
            <animate attributeName='opacity' from='0' to='1' dur='2s'/>\
            <animate attributeName='x' from='0' to='5' begin='1s' dur='4s'/>\
        </rect>",
    );
    // The second track begins at 1s and runs for 4s, ending at 5s.
    assert_eq!(tree.animation_duration(), Some(5.0));
}

#[test]
fn indefinite_repeat_reports_a_single_loop() {
    let tree = parse(
        "<rect width='10' height='10'><animate attributeName='opacity' from='0' to='1' dur='2s' repeatCount='indefinite'/></rect>",
    );
    assert!(tree.has_animations());
    // An infinite animation reports the length of one loop, not infinity.
    assert_eq!(tree.animation_duration(), Some(2.0));
}

#[test]
fn css_animation_duration_includes_delay() {
    let tree = parse(
        "<style>@keyframes fade { from { opacity: 0; } to { opacity: 1; } } #box { animation: fade 3s linear 1s infinite; }</style><rect id='box' width='10' height='10'/>",
    );
    assert!(tree.has_animations());
    // 1s delay + 3s duration; infinite iteration collapses to one loop.
    assert_eq!(tree.animation_duration(), Some(4.0));
}
