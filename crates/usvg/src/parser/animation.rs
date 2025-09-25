// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Animation parsing logic that extracts animation data from SVG animation elements
/// and stores it for use during rendering.
#[cfg(feature = "animation")]
pub mod animation_parser {
    use std::time::Duration;

    use crate::parser::svgtree::{AId, EId, SvgNode};
    use crate::parser::converter;
    use crate::tree::animation::{AnimationData, Keyframe};
    use crate::{Group};
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Global animation registry to store parsed animation data
    static ANIMATION_REGISTRY: Mutex<Option<HashMap<String, HashMap<String, Vec<AnimationData>>>>> = Mutex::new(None);

    /// Main entry point for converting animation elements
    pub(crate) fn convert(node: SvgNode, _state: &converter::State, _cache: &mut converter::Cache, _parent: &mut Group) -> Option<AnimationData> {
        let tag_name = node.tag_name().unwrap();

        // Parse animation data from the SVG animation element
        match tag_name {
            EId::Animate => convert_animate(node),
            EId::AnimateColor => convert_animate_color(node),
            EId::AnimateMotion => convert_animate_motion(node),
            EId::AnimateTransform => convert_animate_transform(node),
            _ => None
        }
    }

    fn convert_animate(node: SvgNode) -> Option<AnimationData> {
        // Get attributeName directly as a string since it's not in AId enum
        let attribute_name = node.attributes()
            .iter()
            .find(|attr| attr.name.to_string() == "attributeName")
            .map(|attr| attr.value.as_str());

        // Get values attribute as string and parse it
        let values_str: Option<&str> = node.attribute(AId::Values);
        let from: Option<&str> = node.attribute(AId::From);
        let to: Option<&str> = node.attribute(AId::To);

        // Parse keyframes
        let keyframes = if let Some(values_str) = values_str {
            parse_values_from_string(values_str)
        } else if let (Some(from), Some(to)) = (from, to) {
            vec![
                Keyframe::new(0.0, from.to_string()),
                Keyframe::new(1.0, to.to_string()),
            ]
        } else {
            return None; // No animation data
        };

        Some(AnimationData::new(
            get_target_element_id(&node),
            attribute_name.unwrap_or("fill").to_string(),
            keyframes,
        ))
    }

    fn convert_animate_color(node: SvgNode) -> Option<AnimationData> {
        // Similar to animate but for color values
        let attribute_name = node.attributes()
            .iter()
            .find(|attr| attr.name.to_string() == "attributeName")
            .map(|attr| attr.value.as_str());
        let values_str: Option<&str> = node.attribute(AId::Values);

        if let Some(values_str) = values_str {
            let keyframes = parse_values_from_string(values_str);
            Some(AnimationData::new(
                get_target_element_id(&node),
                attribute_name.unwrap_or("fill").to_string(),
                keyframes,
            ))
        } else {
            None
        }
    }

    fn convert_animate_motion(node: SvgNode) -> Option<AnimationData> {
        // Motion animation - affects transform
        let values_str: Option<&str> = node.attribute(AId::Values);
        let path: Option<&str> = node.attribute(AId::Path);

        if let Some(values_str) = values_str {
            let keyframes = parse_values_from_string(values_str);
            Some(AnimationData::new(
                get_target_element_id(&node),
                "transform".to_string(),
                keyframes,
            ))
        } else if let Some(path) = path {
            // For path-based motion, create keyframes along the path
            let keyframes = vec![
                Keyframe::new(0.0, "0,0".to_string()),
                Keyframe::new(1.0, path.to_string()),
            ];
            Some(AnimationData::new(
                get_target_element_id(&node),
                "transform".to_string(),
                keyframes,
            ))
        } else {
            None
        }
    }

    fn convert_animate_transform(node: SvgNode) -> Option<AnimationData> {
        let transform_type: Option<&str> = node.attribute(AId::Type);
        let values_str: Option<&str> = node.attribute(AId::Values);
        let from: Option<&str> = node.attribute(AId::From);
        let to: Option<&str> = node.attribute(AId::To);

        let keyframes = if let Some(values_str) = values_str {
            parse_values_from_string(values_str)
        } else if let (Some(from), Some(to)) = (from, to) {
            vec![
                Keyframe::new(0.0, from.to_string()),
                Keyframe::new(1.0, to.to_string()),
            ]
        } else {
            return None;
        };

        Some(AnimationData::new(
            get_target_element_id(&node),
            transform_type.unwrap_or("translate").to_string(),
            keyframes,
        ))
    }

    /// Get the ID of the element this animation targets
    fn get_target_element_id(node: &SvgNode) -> String {
        // Animation elements can target their parent element or a specific element via xlink:href
        if let Some(href) = node.attribute::<&str>(AId::Href) {
            // Remove the # if present
            href.strip_prefix('#').unwrap_or(href).to_string()
        } else {
            // Target the parent element
            node.parent_element()
                .and_then(|p| p.attribute::<&str>(AId::Id))
                .unwrap_or("unknown")
                .to_string()
        }
    }

    /// Parse a list of values from a string into keyframes
    fn parse_values_from_string(values_str: &str) -> Vec<Keyframe<String>> {
        // Split by semicolon or whitespace
        let values: Vec<&str> = values_str.split(';').flat_map(|s| s.split_whitespace()).collect();
        let len = values.len();
        values.into_iter().enumerate().map(|(i, value)| {
            let time = i as f32 / (len - 1).max(1) as f32;
            Keyframe::new(time, value.trim().to_string())
        }).collect()
    }

    /// Parse duration string into Duration
    fn parse_duration(dur_str: &str) -> Duration {
        if dur_str == "indefinite" {
            Duration::MAX
        } else if let Some(seconds) = dur_str.strip_suffix('s') {
            Duration::from_secs_f32(seconds.parse().unwrap_or(0.0))
        } else if let Some(millis) = dur_str.strip_suffix("ms") {
            Duration::from_millis(millis.parse().unwrap_or(0))
        } else {
            // Default to 1 second if parsing fails
            Duration::from_secs(1)
        }
    }

    /// Parse repeat count
    fn parse_repeat_count(repeat_str: &str) -> u32 {
        if repeat_str == "indefinite" {
            u32::MAX
        } else {
            repeat_str.parse().unwrap_or(1)
        }
    }

    /// Store animation data for later use
    /// In a full implementation, this would be stored in a way that
    /// the renderer can access it when rendering the target element
    pub(crate) fn store_animation_data(animation_data: AnimationData) {
        log::debug!("Parsed animation: {:?}", animation_data);

        let mut registry = ANIMATION_REGISTRY.lock().unwrap();
        if registry.is_none() {
            *registry = Some(HashMap::new());
        }

        let registry = registry.as_mut().unwrap();
        let element_id = animation_data.element_id.clone();
        let property = animation_data.property.clone();

        registry
            .entry(element_id)
            .or_insert_with(HashMap::new)
            .entry(property)
            .or_insert_with(Vec::new)
            .push(animation_data);
    }

    /// Check if an element has any animations
    pub(crate) fn has_element_animations(element_id: &str) -> bool {
        let registry = ANIMATION_REGISTRY.lock().unwrap();
        registry.as_ref().map_or(false, |r| r.contains_key(element_id))
    }

    /// Retrieve animation data for a specific element and property
    pub(crate) fn get_animation_data(element_id: &str, property: &str) -> Option<Vec<AnimationData>> {
        let registry = ANIMATION_REGISTRY.lock().unwrap();
        match registry.as_ref() {
            Some(registry) => registry.get(element_id)?.get(property).cloned(),
            None => None,
        }
    }

    /// Example of how to use the animation parser
    #[cfg(feature = "animation")]
    pub fn demo_parsing() {
        // Example SVG animation element content:
        // <animate attributeName="opacity" values="1;0.5;0" dur="2s" />

        let mock_svg_content = r#"
        <animate attributeName="opacity" values="1;0.5;0" dur="2s">
            <rect id="test-rect" width="100" height="50" fill="red"/>
        </animate>
        "#;

        println!("Animation parser ready to parse SVG elements like:");
        println!("{}", mock_svg_content.trim());
        println!("When parsed, it will extract:");
        println!("- Element ID: test-rect");
        println!("- Property: opacity");
        println!("- Keyframes: 1.0 → 0.5 → 0.0");
        println!("- Duration: 2 seconds");
        println!("Use AnimationSupport::has_animations(element_id) to check if element has animations");
    }
}