// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::parser::converter;
use crate::parser::svgtree::{AId, EId, NodeId, SvgNode};

pub(super) fn has_remote_text_animation(all_animations: &[(NodeId, SvgNode)]) -> bool {
    all_animations.iter().any(|(_, animation)| {
        animation
            .try_attribute::<SvgNode>(AId::Href)
            .is_some_and(|target| target.tag_name() == Some(EId::Text))
            || animation
                .attribute::<&str>(AId::Href)
                .is_some_and(|href| href.contains("text"))
    })
}

pub(crate) fn has_display_or_visibility_animation(node: SvgNode, state: &converter::State) -> bool {
    super::targets::animation_nodes(node, state.all_animations)
        .into_iter()
        .any(|animation| {
            matches!(
                animation.attribute::<&str>(AId::AttributeName),
                Some("display" | "visibility")
            )
        })
}

pub(crate) fn can_be_revealed_by_display_animation(
    node: SvgNode,
    state: &converter::State,
) -> bool {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate.attribute(AId::Display) == Some("none")
            && super::has_display_or_visibility_animation(candidate, state)
        {
            return true;
        }
        current = candidate.parent();
    }
    false
}

pub(crate) fn has_paint_animation(node: SvgNode, state: &converter::State, names: &[&str]) -> bool {
    super::targets::animation_nodes(node, state.all_animations)
        .into_iter()
        .any(|animation| {
            animation
                .attribute::<&str>(AId::AttributeName)
                .is_some_and(|name| names.contains(&name))
        })
}

pub(crate) fn base_hidden(node: SvgNode) -> bool {
    node.attribute(AId::Display) == Some("none")
}

pub(crate) fn warn_text_animations(node: SvgNode, state: &converter::State) {
    let nested = node
        .descendants()
        .any(|child| child.tag_name().is_some_and(|tag| tag.is_animation()));
    let remote = state.all_animations.iter().any(|(_, animation)| {
        animation
            .try_attribute::<SvgNode>(AId::Href)
            .is_some_and(|target| target == node)
    });
    if nested || remote {
        log::warn!("Animation of text elements is not supported.");
    }
}

pub(crate) fn warn_filter_content_animations(node: SvgNode, state: &converter::State) {
    let nested = node
        .descendants()
        .any(|child| child.tag_name().is_some_and(|tag| tag.is_animation()));
    let remote = state.all_animations.iter().any(|(_, animation)| {
        let Some(target) = animation.try_attribute::<SvgNode>(AId::Href) else {
            return false;
        };
        target.ancestors().any(|ancestor| ancestor == node)
    });
    if nested || remote {
        log::warn!("Animation of filter content is not supported.");
    }
}
