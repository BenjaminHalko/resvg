// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod base_value;
mod geometry;
mod parsing;
mod targets;
mod warnings;

pub(crate) use targets::{
    collect_animations, collect_node_animations, collect_view_box_animation, image_root_animations,
    renderable_animations, synthesized_path, wrapper_animations,
};
pub(crate) use warnings::{
    base_hidden, can_be_revealed_by_display_animation, has_display_or_visibility_animation,
    has_paint_animation, warn_filter_content_animations, warn_text_animations,
};
