// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(feature = "animation")]

use std::sync::{Mutex, Once, OnceLock};

use usvg::{AnimationKind, Dur, Node, Options, Timing, Tree};

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

fn parse(body: &str) -> Tree {
    Tree::from_str(&format!("<svg xmlns='{NS}' width='20' height='20'>{body}</svg>"), &Options::default())
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
    let tree = parse("<g><animateTransform attributeName='transform' type='translate' from='0 0' to='2 3' dur='1s'/><rect width='4' height='4'/></g>");
    let group = group(&tree.root().children()[0]);
    assert!(matches!(group.animations()[0].kind(), AnimationKind::Transform(_)));
}

#[test]
fn shape_transform_and_opacity_use_wrapper_groups() {
    let transform = parse("<rect width='4' height='4'><animateTransform attributeName='transform' type='translate' from='0 0' to='2 3' dur='1s'/></rect>");
    let transform_group = group(&transform.root().children()[0]);
    assert!(matches!(transform_group.animations()[0].kind(), AnimationKind::Transform(_)));
    assert!(matches!(transform_group.children()[0], Node::Path(_)));

    let opacity = parse("<rect width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s'/></rect>");
    let opacity_group = group(&opacity.root().children()[0]);
    assert!(matches!(opacity_group.animations()[0].kind(), AnimationKind::Opacity(_)));
    assert!(matches!(opacity_group.children()[0], Node::Path(_)));
}

#[test]
fn image_geometry_tracks_attach_to_image_root() {
    let tree = parse(&format!("<image href='{PNG}' y='1' width='4' height='4'><animate attributeName='y' from='1' to='3' dur='2s'/></image>"));
    let root = group(&tree.root().children()[0]);
    assert!(matches!(root.animations()[0].kind(), AnimationKind::ImageY(_)));
    assert!(matches!(root.children()[0], Node::Image(_)));
}

#[test]
fn concurrent_image_geometry_tracks_retain_timing() {
    let tree = parse(&format!("<image href='{PNG}' width='4' height='4'><animate attributeName='width' from='4' to='8' dur='2s'/><animate attributeName='height' from='4' to='9' begin='1s' dur='4s'/></image>"));
    let animation = group(&tree.root().children()[0]).animation().unwrap();
    assert_eq!(animation.animations().len(), 2);
    assert!(matches!(animation.animations()[0].kind(), AnimationKind::ImageWidth(_)));
    assert!(matches!(animation.animations()[1].kind(), AnimationKind::ImageHeight(_)));
    assert!(matches!(animation.animations()[0].timing(), Timing::Smil(timing) if matches!(timing.dur(), Dur::Seconds(value) if *value == 2.0)));
    assert!(matches!(animation.animations()[1].timing(), Timing::Smil(timing) if matches!(timing.dur(), Dur::Seconds(value) if *value == 4.0)));
}

#[test]
fn mask_content_animation_is_attached() {
    let tree = parse("<defs><mask id='mask'><rect width='4' height='4'><animate attributeName='opacity' from='0' to='1' dur='1s'/></rect></mask></defs><rect width='4' height='4' mask='url(#mask)'/>");
    let mask = group(&tree.root().children()[0]).mask().unwrap();
    let animated = group(&mask.root().children()[0]);
    assert!(matches!(animated.animations()[0].kind(), AnimationKind::Opacity(_)));
}

#[test]
fn display_none_reveal_nodes_are_retained_in_tree_and_clip_path() {
    let tree = parse("<defs><clipPath id='clip'><rect display='none' width='4' height='4'><set attributeName='display' to='inline' begin='1s'/></rect></clipPath></defs><rect display='none' width='4' height='4'><set attributeName='display' to='inline' begin='1s'/></rect><rect width='4' height='4' clip-path='url(#clip)'/>");
    let main = path(&tree.root().children()[0]).animation().unwrap();
    assert!(main.base_hidden());
    let clip = group(&tree.root().children()[1]).clip_path().unwrap();
    let clipped = path(&clip.root().children()[0]).animation().unwrap();
    assert!(clipped.base_hidden());
}

#[test]
fn paint_and_stroke_carriers_preserve_disabled_static_state() {
    let fill = parse("<path d='M0 0 L4 4' fill='none'><animate attributeName='fill' from='red' to='blue' dur='1s'/></path>");
    let fill_carrier = path(&fill.root().children()[0]).animation().unwrap().fill().unwrap();
    assert!(fill_carrier.underlying_disabled());

    let stroke = parse("<path d='M0 0 L4 4' stroke='red' stroke-width='0'><animate attributeName='stroke-width' from='0' to='10' dur='1s'/></path>");
    let stroke_carrier = path(&stroke.root().children()[0]).animation().unwrap().stroke().unwrap();
    assert!(stroke_carrier.paint().is_some());
    assert_eq!(stroke_carrier.width(), 0.0);
}

#[test]
fn zero_static_geometry_uses_a_path_animation_carrier() {
    let tree = parse("<rect width='0' height='4'><animate attributeName='width' from='0' to='8' dur='1s'/></rect>");
    let animation = path(&tree.root().children()[0]).animation().unwrap();
    assert!(!animation.path().unwrap().underlying_renderable());
    assert!(matches!(animation.animations()[0].kind(), AnimationKind::Path(_)));
}

#[test]
fn text_animation_warns_without_attachment() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        WARNINGS.get_or_init(|| Mutex::new(Vec::new()));
        log::set_logger(&CaptureLogger).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    WARNINGS.get().unwrap().lock().unwrap().clear();
    let tree = parse("<text x='0' y='10'>text<animate attributeName='opacity' from='0' to='1' dur='1s'/></text><rect width='4' height='4'/>");
    assert!(matches!(tree.root().children()[0], Node::Path(_)));
    assert!(WARNINGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|warning| warning == "Animation of text elements is not supported."));
}
