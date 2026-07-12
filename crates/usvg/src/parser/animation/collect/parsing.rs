// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use super::super::motion::parse_animate_motion;
use super::super::timing::{parse_easing, parse_smil_timing};
use super::super::values::{parse_smil_transform_values, parse_smil_values};
use super::base_value::base_value;
use super::geometry::{is_shape_geometry, parse_geometry_animation};
use super::targets::map_target_kind;
use crate::parser::converter;
use crate::parser::svgtree::{AId, EId, NodeId, SvgNode};
use crate::tree::animation::{
    Accumulate, Additive, Animation, AnimationSource, Easing, Timing, TransformKind,
};

pub(super) fn parse_animation(
    target: SvgNode,
    node: SvgNode,
    all_animations: &[(NodeId, SvgNode)],
    state: &converter::State,
    _cache: &mut converter::Cache,
) -> Option<Arc<Animation>> {
    let tag = node.tag_name()?;
    let timing = Timing::Smil(parse_smil_timing(node, all_animations));
    let additive = additive(node);
    let accumulate = accumulate(node);
    let attribute_name = node.attribute::<&str>(AId::AttributeName);

    let (kind, easing, additive, accumulate) = match tag {
        EId::AnimateMotion => {
            let (kind, easing) = parse_animate_motion(node)?;
            (kind, easing, additive, accumulate)
        }
        EId::AnimateTransform => {
            let kind = parse_transform_kind(node.attribute(AId::Type)?)?;
            let count = value_count(node, false);
            let easing = parse_easing(node, count)?;
            let values = parse_smil_transform_values(
                kind,
                false,
                node.attribute(AId::Values),
                node.attribute(AId::From),
                node.attribute(AId::To),
                node.attribute(AId::By),
                additive,
                accumulate,
                easing.calc_mode(),
                easing.key_times(),
                None,
            )?;
            (values.kind, easing, values.additive, values.accumulate)
        }
        EId::Animate | EId::AnimateColor | EId::Set => {
            let attribute_name = attribute_name?;
            let is_set = tag == EId::Set;
            let count = value_count(node, is_set);
            let easing = if is_set {
                Easing::new(crate::CalcMode::Discrete, None, None)
            } else {
                parse_easing(node, count)?
            };
            let values = if is_shape_geometry(target, attribute_name) {
                parse_geometry_animation(
                    target,
                    node,
                    attribute_name,
                    is_set,
                    additive,
                    accumulate,
                    &easing,
                    state,
                )?
            } else {
                parse_smil_values(
                    attribute_name,
                    if is_set {
                        node.attribute(AId::To)
                            .or_else(|| node.attribute(AId::Values))
                    } else {
                        node.attribute(AId::Values)
                    },
                    if is_set {
                        None
                    } else {
                        node.attribute(AId::From)
                    },
                    if is_set {
                        None
                    } else {
                        node.attribute(AId::To)
                    },
                    if is_set {
                        None
                    } else {
                        node.attribute(AId::By)
                    },
                    additive,
                    accumulate,
                    easing.calc_mode(),
                    easing.key_times(),
                    &base_value(target, attribute_name, state),
                )?
            };
            (values.kind, easing, values.additive, values.accumulate)
        }
        _ => return None,
    };

    let attribute_name = attribute_name.unwrap_or("");
    let kind = map_target_kind(target, attribute_name, kind, state);
    let suppressed_by_important = AId::from_str(attribute_name).is_some_and(|attribute| {
        target
            .attributes()
            .iter()
            .find(|item| item.name == attribute)
            .is_some_and(|item| item.important)
    });
    Some(Arc::new(Animation::new(
        kind,
        timing,
        easing,
        additive,
        accumulate,
        AnimationSource::Smil,
        suppressed_by_important,
    )))
}

fn parse_transform_kind(value: &str) -> Option<TransformKind> {
    match value {
        "translate" => Some(TransformKind::Translate),
        "scale" => Some(TransformKind::Scale),
        "rotate" => Some(TransformKind::Rotate),
        "skewX" => Some(TransformKind::SkewX),
        "skewY" => Some(TransformKind::SkewY),
        _ => None,
    }
}

fn additive(node: SvgNode) -> Additive {
    if node.attribute(AId::Additive) == Some("sum") {
        Additive::Sum
    } else {
        Additive::Replace
    }
}

fn accumulate(node: SvgNode) -> Accumulate {
    if node.attribute(AId::Accumulate) == Some("sum") {
        Accumulate::Sum
    } else {
        Accumulate::None
    }
}

fn value_count(node: SvgNode, is_set: bool) -> usize {
    if is_set {
        return 1;
    }
    if let Some(values) = node.attribute::<&str>(AId::Values) {
        return values
            .split(';')
            .filter(|value| !value.trim().is_empty())
            .count();
    }
    if node.has_attribute(AId::To) || node.has_attribute(AId::By) {
        2
    } else {
        1
    }
}
