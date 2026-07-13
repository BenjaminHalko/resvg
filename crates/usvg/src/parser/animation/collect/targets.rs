// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use svgtypes::Length;

use super::super::css;
use super::parsing::parse_animation;
use super::warnings::has_remote_text_animation;
use crate::parser::converter;
use crate::parser::svgtree::{AId, EId, NodeId, SvgNode};
use crate::tree::animation::{
    Additive, Animation, AnimationKind, Track, TransformFunction, ViewBoxAnimation,
};

pub(crate) fn collect_node_animations(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<Arc<Animation>> {
    if has_remote_text_animation(state.all_animations) {
        log::warn!("Animation of text elements is not supported.");
    }
    let mut animations = super::collect_animations(node, state.all_animations, state, cache);
    animations.extend(css::build_css_animations(node, node.document(), state));
    animations
}

pub(crate) fn collect_animations(
    node: SvgNode,
    all_animations: &[(NodeId, SvgNode)],
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<Arc<Animation>> {
    animation_nodes(node, all_animations)
        .into_iter()
        .filter_map(|animation| parse_animation(node, animation, all_animations, state, cache))
        .collect()
}

pub(crate) fn wrapper_animations(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<Arc<Animation>> {
    let group_target = matches!(node.tag_name(), Some(EId::G | EId::Svg | EId::Use));
    collect_node_animations(node, state, cache)
        .into_iter()
        .filter(|animation| group_target || is_wrapper_kind(animation.kind()))
        .collect()
}

pub(crate) fn renderable_animations(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<Arc<Animation>> {
    collect_node_animations(node, state, cache)
        .into_iter()
        .filter(|animation| {
            !is_wrapper_kind(animation.kind()) && !is_image_geometry_kind(animation.kind())
        })
        .collect()
}

pub(crate) fn image_root_animations(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<Arc<Animation>> {
    collect_node_animations(node, state, cache)
        .into_iter()
        .filter(|animation| is_image_geometry_kind(animation.kind()))
        .collect()
}

pub(crate) fn collect_view_box_animation(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<ViewBoxAnimation> {
    let static_aspect: svgtypes::AspectRatio =
        node.attribute(AId::PreserveAspectRatio).unwrap_or_default();
    let mut result = None;
    for animation in collect_node_animations(node, state, cache) {
        let AnimationKind::ViewBox(track) = animation.kind() else {
            continue;
        };
        if result.is_some() || matches!(animation.additive(), Additive::Sum) {
            log::warn!("Only a single non-additive viewBox animation is supported.");
            continue;
        }
        result = Some(ViewBoxAnimation::new(
            track.clone(),
            static_aspect,
            animation.timing().clone(),
            animation.easing().clone(),
        ));
    }
    result
}

pub(crate) fn synthesized_path(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<Arc<tiny_skia_path::Path>> {
    collect_node_animations(node, state, cache)
        .into_iter()
        .find_map(|animation| match animation.kind() {
            AnimationKind::Path(track) => track
                .keyframes()
                .iter()
                .find(|keyframe| keyframe.renderable())
                .map(|keyframe| keyframe.path.clone()),
            _ => None,
        })
}

pub(super) fn animation_nodes<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
) -> Vec<SvgNode<'a, 'input>> {
    let mut nodes = node
        .children()
        .filter(|child| {
            child.tag_name().is_some_and(|tag| tag.is_animation())
                && !child.has_attribute(AId::Href)
        })
        .collect::<Vec<_>>();
    nodes.extend(all_animations.iter().filter_map(|(_, animation)| {
        animation
            .try_attribute::<SvgNode>(AId::Href)
            .filter(|target| *target == node)
            .map(|_| *animation)
    }));
    nodes
}

pub(super) fn map_target_kind(
    target: SvgNode,
    attribute_name: &str,
    kind: AnimationKind,
    state: &converter::State,
) -> AnimationKind {
    if target.tag_name() == Some(EId::Image) {
        return match (attribute_name, kind) {
            ("x", AnimationKind::GradientGeometry(track)) => AnimationKind::ImageX(track),
            ("y", AnimationKind::GradientGeometry(track)) => AnimationKind::ImageY(track),
            ("width", AnimationKind::GradientGeometry(track)) => AnimationKind::ImageWidth(track),
            ("height", AnimationKind::GradientGeometry(track)) => AnimationKind::ImageHeight(track),
            (_, kind) => kind,
        };
    }
    if target.tag_name() == Some(EId::Use) {
        if let AnimationKind::GradientGeometry(track) = kind {
            let static_x = target.convert_user_length(AId::X, state, Length::zero());
            let static_y = target.convert_user_length(AId::Y, state, Length::zero());
            let keyframes = track
                .keyframes()
                .iter()
                .map(|keyframe| {
                    let (x, y) = match attribute_name {
                        "x" => (*keyframe.value(), static_y),
                        "y" => (static_x, *keyframe.value()),
                        _ => (*keyframe.value(), 0.0),
                    };
                    crate::Keyframe::new(
                        keyframe.offset(),
                        vec![TransformFunction::Translate(x, y)],
                        keyframe.timing_function().cloned(),
                    )
                })
                .collect::<Vec<_>>();
            return AnimationKind::Transform(Track::new(keyframes));
        }
    }
    kind
}

fn is_wrapper_kind(kind: &AnimationKind) -> bool {
    matches!(
        kind,
        AnimationKind::Transform(_) | AnimationKind::Motion(_) | AnimationKind::Opacity(_)
    )
}

fn is_image_geometry_kind(kind: &AnimationKind) -> bool {
    matches!(
        kind,
        AnimationKind::ImageX(_)
            | AnimationKind::ImageY(_)
            | AnimationKind::ImageWidth(_)
            | AnimationKind::ImageHeight(_)
    )
}
