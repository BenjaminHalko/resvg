// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;
use svgtypes::Length;

use super::svgtree::{AId, SvgNode};
use crate::tree::{AnimatedValue, Keyframe};

/// Parses animation-related attributes from an SVG node.
///
/// This function looks for animation attributes like `animate`, `animateTransform`,
/// `animateColor`, etc. and parses them into keyframes.
#[cfg(feature = "animation")]
pub(crate) fn parse_animations(node: SvgNode) -> Vec<(String, AnimatedValue<String>)> {
    let mut animations = Vec::new();

    // Look for animation elements that target this node
    for child in node.children() {
        if !is_animation_element(child) {
            continue;
        }

        if let Some((attribute_name, animated_value)) = parse_single_animation(child, node) {
            animations.push((attribute_name, animated_value));
        }
    }

    animations
}

/// Checks if a node is an animation element.
#[cfg(feature = "animation")]
fn is_animation_element(node: SvgNode) -> bool {
    matches!(
        node.tag_name(),
        Some("animate")
            | Some("animateColor")
            | Some("animateTransform")
            | Some("animateMotion")
    )
}

/// Parses a single animation element.
#[cfg(feature = "animation")]
fn parse_single_animation(animation_node: SvgNode, target_node: SvgNode) -> Option<(String, AnimatedValue<String>)> {
    // Get the attribute being animated
    let attribute_name = animation_node.attribute::<&str>(AId::AttributeName)?;

    // Parse the animation values
    let values = parse_animation_values(animation_node, target_node)?;
    let animated_value = AnimatedValue::animated(values);

    Some((attribute_name.to_string(), animated_value))
}

/// Parses the values from an animation element into keyframes.
#[cfg(feature = "animation")]
fn parse_animation_values(animation_node: SvgNode, _target_node: SvgNode) -> Option<Vec<Keyframe<String>>> {
    // Try different ways animations can specify values

    // 1. 'values' attribute - explicit keyframe values
    if let Some(values_str) = animation_node.attribute::<&str>(AId::Values) {
        return parse_values_attribute(values_str);
    }

    // 2. 'from' and 'to' attributes - simple two-keyframe animation
    if let (Some(from), Some(to)) = (
        animation_node.attribute::<&str>(AId::From),
        animation_node.attribute::<&str>(AId::To),
    ) {
        return Some(vec![
            Keyframe::new(0.0, from.to_string()),
            Keyframe::new(1.0, to.to_string()),
        ]);
    }

    // 3. 'by' attribute (requires 'from' or base value)
    if let Some(by) = animation_node.attribute::<&str>(AId::By) {
        // For now, just create a simple animation
        return Some(vec![
            Keyframe::new(0.0, "0".to_string()), // Would need base value
            Keyframe::new(1.0, by.to_string()),
        ]);
    }

    None
}

/// Parses the 'values' attribute into keyframes.
#[cfg(feature = "animation")]
fn parse_values_attribute(values_str: &str) -> Option<Vec<Keyframe<String>>> {
    let values: Vec<&str> = values_str.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();

    if values.is_empty() {
        return None;
    }

    let count = values.len();
    let mut keyframes = Vec::with_capacity(count);

    for (i, value) in values.iter().enumerate() {
        let offset = i as f32 / (count.max(1) - 1) as f32;
        keyframes.push(Keyframe::new(offset, value.to_string()));
    }

    Some(keyframes)
}

/// Parses timing information from animation elements.
#[cfg(feature = "animation")]
pub(crate) fn parse_timing(animation_node: SvgNode) -> AnimationTiming {
    AnimationTiming {
        duration: animation_node
            .attribute::<&str>(AId::Dur)
            .and_then(parse_duration)
            .unwrap_or(AnimationTiming::default_duration()),
        begin: animation_node
            .attribute::<&str>(AId::Begin)
            .and_then(parse_begin_time)
            .unwrap_or(0.0),
        end: animation_node
            .attribute::<&str>(AId::End)
            .and_then(parse_end_time),
        repeat_count: animation_node
            .attribute::<&str>(AId::RepeatCount)
            .and_then(parse_repeat_count)
            .unwrap_or(1.0),
    }
}

/// Animation timing information.
#[cfg(feature = "animation")]
#[derive(Clone, Debug)]
pub(crate) struct AnimationTiming {
    pub(crate) duration: f32,
    pub(crate) begin: f32,
    pub(crate) end: Option<f32>,
    pub(crate) repeat_count: f32,
}

#[cfg(feature = "animation")]
impl AnimationTiming {
    pub(crate) fn default_duration() -> f32 {
        1.0 // Default to 1 second
    }

    pub(crate) fn is_active_at(&self, time: f32) -> bool {
        if time < self.begin {
            return false;
        }

        if let Some(end) = self.end {
            if time > end {
                return false;
            }
        }

        true
    }
}

/// Parses duration strings like "1s", "500ms", "indefinite".
#[cfg(feature = "animation")]
fn parse_duration(duration_str: &str) -> Option<f32> {
    if duration_str == "indefinite" {
        return Some(f32::INFINITY);
    }

    if let Some(stripped) = duration_str.strip_suffix("ms") {
        return stripped.parse().ok().map(|ms: f32| ms / 1000.0);
    }

    if let Some(stripped) = duration_str.strip_suffix('s') {
        return stripped.parse().ok();
    }

    // Try parsing as seconds if no unit specified
    duration_str.parse().ok()
}

/// Parses begin time strings.
#[cfg(feature = "animation")]
fn parse_begin_time(begin_str: &str) -> Option<f32> {
    if begin_str == "indefinite" {
        return Some(f32::INFINITY);
    }

    parse_duration(begin_str)
}

/// Parses end time strings.
#[cfg(feature = "animation")]
fn parse_end_time(end_str: &str) -> Option<f32> {
    if end_str == "indefinite" {
        return None;
    }

    parse_duration(end_str)
}

/// Parses repeat count.
#[cfg(feature = "animation")]
fn parse_repeat_count(repeat_str: &str) -> Option<f32> {
    if repeat_str == "indefinite" {
        return Some(f32::INFINITY);
    }

    repeat_str.parse().ok()
}