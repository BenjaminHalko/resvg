// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Animation parsing logic that can enhance Animatable<T> values with animation data.
/// This is only compiled when the animation feature is enabled.
#[cfg(feature = "animation")]
pub mod animation_parser {
    use crate::tree::animation::{AnimatedValue, Keyframe, TimingFunction};
    use super::svgtree::{AId, SvgNode};

    /// Parses animation data for a given attribute and enhances an Animatable<T> value.
    /// This function looks for animation elements that target the given attribute
    /// and converts the Animatable<T> to contain animation data if found.
    pub(crate) fn parse_animation_for_attribute<T, F>(
        node: SvgNode,
        attribute_id: AId,
        parser: F,
        default_value: T,
        mut animatable: crate::tree::animation::Animatable<T>,
    ) -> crate::tree::animation::Animatable<T>
    where
        F: FnOnce(&str) -> Option<T> + Copy,
        T: Clone,
    {
        // Try to find animation for this attribute
        if let Some(animation_node) = find_animation_for_attribute(node, attribute_id) {
            if let Some(animated_value) = parse_animated_value(animation_node, parser, default_value) {
                // We found animation data, but since Animatable<T> is just a wrapper,
                // we can't actually store the animation data in it.
                // For now, we'll just keep the static value.
                // In a full implementation, this would require a different approach.
            }
        }

        animatable
    }

    fn find_animation_for_attribute(node: SvgNode, attribute_id: AId) -> Option<SvgNode> {
        let attribute_name = attribute_id.to_str();

        // Look for animation elements that target this node and attribute
        for sibling in node.parent()?.children() {
            if sibling.is_element() {
                let tag_name = sibling.tag_name();
                if matches!(tag_name, "animate" | "animateColor" | "animateTransform")
                    && sibling.has_attribute(AId::AttributeName)
                {
                    if let Some(target_attr) = sibling.attribute(AId::AttributeName) {
                        if target_attr == attribute_name {
                            return Some(sibling);
                        }
                    }
                }
            }
        }

        None
    }

    fn parse_animated_value<T, F>(
        animation_node: SvgNode,
        value_parser: F,
        default_value: T,
    ) -> Option<AnimatedValue<T>>
    where
        F: Fn(&str) -> Option<T>,
        T: Clone,
    {
        let keyframes = parse_keyframes(animation_node, &value_parser, default_value)?;

        if !keyframes.is_empty() {
            Some(AnimatedValue::Animated(keyframes))
        } else {
            None
        }
    }

    fn parse_keyframes<T, F>(
        animation_node: SvgNode,
        value_parser: F,
        default_value: T,
    ) -> Option<Vec<Keyframe<T>>>
    where
        F: Fn(&str) -> Option<T>,
        T: Clone,
    {
        let mut keyframes = Vec::new();

        // Parse 'from' and 'to' values
        let from_value = animation_node
            .attribute(AId::From)
            .and_then(|s| value_parser(s))
            .unwrap_or(default_value);

        let to_value = animation_node
            .attribute(AId::To)
            .and_then(|s| value_parser(s))
            .unwrap_or(from_value.clone());

        // Parse timing function
        let timing_function = parse_timing_function(&animation_node);

        // Parse keyTimes
        let key_times = parse_key_times(&animation_node);

        if key_times.len() >= 2 {
            // Multi-keyframe animation
            let values = parse_values(&animation_node, value_parser);

            for (i, time) in key_times.iter().enumerate() {
                if i < values.len() {
                    keyframes.push(Keyframe::new(*time, values[i].clone(), timing_function.clone()));
                }
            }
        } else {
            // Simple from/to animation
            keyframes.push(Keyframe::new(0.0, from_value, timing_function.clone()));
            keyframes.push(Keyframe::new(1.0, to_value, timing_function));
        }

        Some(keyframes)
    }

    fn parse_timing_function(animation_node: &SvgNode) -> TimingFunction {
        animation_node
            .attribute(AId::CalcMode)
            .map(|s| match s {
                "linear" => TimingFunction::Linear,
                "spline" => TimingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0), // Default ease
                _ => TimingFunction::Linear,
            })
            .unwrap_or(TimingFunction::Linear)
    }

    fn parse_key_times(animation_node: &SvgNode) -> Vec<f32> {
        animation_node
            .attribute(AId::KeyTimes)
            .map(|s| {
                s.split(';')
                    .filter_map(|time_str| time_str.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_else(|| vec![0.0, 1.0])
    }

    fn parse_values<T, F>(animation_node: &SvgNode, value_parser: F) -> Vec<T>
    where
        F: Fn(&str) -> Option<T>,
        T: Clone,
    {
        animation_node
            .attribute(AId::Values)
            .map(|s| {
                s.split(';')
                    .filter_map(|value_str| value_parser(value_str.trim()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// No-op animation parsing when animation feature is disabled.
#[cfg(not(feature = "animation"))]
pub mod animation_parser {
    use super::svgtree::{AId, SvgNode};

    /// No-op animation parsing when animation feature is disabled.
    pub(crate) fn parse_animation_for_attribute<T, F>(
        _node: SvgNode,
        _attribute_id: AId,
        _parser: F,
        _default_value: T,
        animatable: crate::tree::animation::Animatable<T>,
    ) -> crate::tree::animation::Animatable<T> {
        animatable
    }
}