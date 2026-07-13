// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::{Group, Node, Rect};

/// Bakes each CSS transform origin after static bounds have been calculated.
pub(crate) fn bake_transform_origins(root: &mut Group) {
    bake_group(root);
}

fn bake_group(group: &mut Group) {
    let fill_bounds = group.bounding_box();
    let stroke_bounds = group.stroke_bounding_box();
    bake_animations(group.animation.as_deref_mut(), fill_bounds, stroke_bounds);
    for node in &mut group.children {
        match node {
            Node::Group(child) => bake_group(child),
            Node::Path(path) => {
                let fill_bounds = path.bounding_box();
                let stroke_bounds = path.stroke_bounding_box();
                bake_animations(path.animation.as_deref_mut(), fill_bounds, stroke_bounds);
            }
            Node::Image(image) => {
                let bounds = image.bounding_box();
                bake_animations(image.animation.as_deref_mut(), bounds, bounds);
            }
            Node::Text(_) => {}
        }
    }
}

fn bake_animations(
    animations: Option<&mut crate::tree::animation::NodeAnimation>,
    fill_bounds: Rect,
    stroke_bounds: Rect,
) {
    let Some(animations) = animations else {
        return;
    };
    for animation in &mut animations.animations {
        Arc::make_mut(animation).bake_css_origin(fill_bounds, stroke_bounds);
    }
}
