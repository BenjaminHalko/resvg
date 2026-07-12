// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;
use std::sync::Arc;

use svgtypes::Length;

use super::geom::{ShapeGeometry, bake_geometry_animation};
use super::motion::parse_animate_motion;
use super::timing::parse_easing;
use super::values::{BaseValue, SmilValues, parse_smil_transform_values, parse_smil_values};
use crate::parser::converter;
use crate::parser::svgtree::{AId, EId, NodeId, SvgNode};
use crate::tree::animation::{
    Accumulate, Additive, Animation, AnimationKind, AnimationSource, AnimationVisibility, Easing,
    Timing, TransformKind, TransformTrack, ViewBoxAnimation,
};
use crate::{Opacity, StrokeMiterlimit, Visibility};

pub(crate) fn collect_node_animations(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<Arc<Animation>> {
    if has_remote_text_animation(state.all_animations) {
        log::warn!("Animation of text elements is not supported.");
    }
    let mut animations = collect_animations(node, state.all_animations, state, cache);
    animations.extend(super::css::build_css_animations(node, node.document()));
    animations
}

fn has_remote_text_animation(all_animations: &[(NodeId, SvgNode)]) -> bool {
    all_animations.iter().any(|(_, animation)| {
        animation
            .try_attribute::<SvgNode>(AId::Href)
            .is_some_and(|target| target.tag_name() == Some(EId::Text))
    })
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

pub(crate) fn has_display_or_visibility_animation(node: SvgNode, state: &converter::State) -> bool {
    animation_nodes(node, state.all_animations)
        .into_iter()
        .any(|animation| {
            matches!(
                animation.attribute::<&str>(AId::AttributeName),
                Some("display" | "visibility")
            )
        })
}

pub(crate) fn has_paint_animation(node: SvgNode, state: &converter::State, names: &[&str]) -> bool {
    animation_nodes(node, state.all_animations)
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

fn animation_nodes<'a, 'input: 'a>(
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

fn parse_animation(
    target: SvgNode,
    node: SvgNode,
    all_animations: &[(NodeId, SvgNode)],
    state: &converter::State,
    _cache: &mut converter::Cache,
) -> Option<Arc<Animation>> {
    let tag = node.tag_name()?;
    let timing = Timing::Smil(super::timing::parse_smil_timing(node, all_animations));
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
                let values = parse_smil_values(
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
                )?;
                values
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

fn parse_geometry_animation(
    target: SvgNode,
    node: SvgNode,
    attribute_name: &str,
    is_set: bool,
    additive: Additive,
    accumulate: Accumulate,
    easing: &Easing,
    state: &converter::State,
) -> Option<SmilValues> {
    let geometry = shape_geometry(target, state);
    let (values, offsets, raw_values) = if matches!(attribute_name, "d" | "points") {
        let values = raw_geometry_values(target, node, attribute_name, is_set)?;
        let offsets = offsets(values.len(), easing.key_times());
        (Vec::new(), offsets, Some(values))
    } else {
        let values = parse_smil_values(
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
        )?;
        let AnimationKind::GradientGeometry(track) = values.kind else {
            return None;
        };
        let offsets = track
            .keyframes()
            .iter()
            .map(|keyframe| keyframe.offset())
            .collect();
        let values = track
            .keyframes()
            .iter()
            .map(|keyframe| *keyframe.value())
            .collect();
        (values, offsets, None)
    };
    let key_timing_fns = vec![None; offsets.len()];
    let bake = bake_geometry_animation(
        target.tag_name()?,
        attribute_name,
        geometry,
        &values,
        &offsets,
        &key_timing_fns,
        easing.calc_mode(),
        accumulate,
        raw_values.as_deref().filter(|_| attribute_name == "d"),
        raw_values.as_deref().filter(|_| attribute_name == "points"),
    )?;
    Some(SmilValues {
        kind: bake.kind,
        additive,
        accumulate,
        calc_mode: bake.calc_mode,
    })
}

fn raw_geometry_values<'a, 'input: 'a>(
    target: SvgNode<'a, 'input>,
    node: SvgNode<'a, 'input>,
    name: &str,
    is_set: bool,
) -> Option<Vec<&'a str>> {
    if is_set {
        return node
            .attribute::<&str>(AId::To)
            .or_else(|| node.attribute::<&str>(AId::Values))
            .map(|value| vec![value.trim()]);
    }
    if let Some(values) = node.attribute::<&str>(AId::Values) {
        let values = values
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect();
        return Some(values);
    }
    match (node.attribute(AId::From), node.attribute(AId::To)) {
        (Some(from), Some(to)) => Some(vec![from, to]),
        (None, Some(to)) => Some(vec![target.attribute(AId::from_str(name)?)?, to]),
        _ => None,
    }
}

fn shape_geometry(node: SvgNode, state: &converter::State) -> ShapeGeometry {
    ShapeGeometry {
        x: node.convert_user_length(AId::X, state, Length::zero()),
        y: node.convert_user_length(AId::Y, state, Length::zero()),
        width: node.convert_user_length(AId::Width, state, Length::zero()),
        height: node.convert_user_length(AId::Height, state, Length::zero()),
        rx: node.convert_user_length(AId::Rx, state, Length::zero()),
        ry: node.convert_user_length(AId::Ry, state, Length::zero()),
        cx: node.convert_user_length(AId::Cx, state, Length::zero()),
        cy: node.convert_user_length(AId::Cy, state, Length::zero()),
        r: node.convert_user_length(AId::R, state, Length::zero()),
        x1: node.convert_user_length(AId::X1, state, Length::zero()),
        y1: node.convert_user_length(AId::Y1, state, Length::zero()),
        x2: node.convert_user_length(AId::X2, state, Length::zero()),
        y2: node.convert_user_length(AId::Y2, state, Length::zero()),
    }
}

fn base_value(node: SvgNode, name: &str, state: &converter::State) -> BaseValue {
    match name {
        "opacity" => BaseValue::Opacity(node.attribute(AId::Opacity).unwrap_or(Opacity::ONE)),
        "fill" | "stroke" => AId::from_str(name)
            .and_then(|attribute| node.find_attribute::<&str>(attribute))
            .and_then(|value| svgtypes::Color::from_str(value).ok())
            .map_or(BaseValue::None, BaseValue::Color),
        "stroke-width" => BaseValue::Number(node.resolve_length(AId::StrokeWidth, state, 1.0)),
        "stroke-dashoffset" => {
            BaseValue::Number(node.resolve_length(AId::StrokeDashoffset, state, 0.0))
        }
        "stroke-dasharray" => BaseValue::Numbers(Vec::new()),
        "stroke-miterlimit" => BaseValue::Miterlimit(StrokeMiterlimit::new(
            node.find_attribute(AId::StrokeMiterlimit).unwrap_or(4.0),
        )),
        "stroke-linecap" => {
            BaseValue::Linecap(node.find_attribute(AId::StrokeLinecap).unwrap_or_default())
        }
        "stroke-linejoin" => {
            BaseValue::Linejoin(node.find_attribute(AId::StrokeLinejoin).unwrap_or_default())
        }
        "fill-rule" => BaseValue::FillRule(node.find_attribute(AId::FillRule).unwrap_or_default()),
        "display" => BaseValue::Boolean(node.attribute(AId::Display) != Some("none")),
        "visibility" => BaseValue::Visibility(
            match node.find_attribute(AId::Visibility).unwrap_or_default() {
                Visibility::Visible => AnimationVisibility::Visible,
                Visibility::Hidden => AnimationVisibility::Hidden,
                Visibility::Collapse => AnimationVisibility::Collapse,
            },
        ),
        "x" | "y" | "width" | "height" | "cx" | "cy" | "r" | "rx" | "ry" | "x1" | "y1" | "x2"
        | "y2" => AId::from_str(name)
            .map(|attribute| node.convert_user_length(attribute, state, Length::zero()))
            .map_or(BaseValue::None, BaseValue::Number),
        _ => BaseValue::None,
    }
}

fn map_target_kind(
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
                    let values = match attribute_name {
                        "x" => vec![*keyframe.value(), static_y],
                        "y" => vec![static_x, *keyframe.value()],
                        _ => vec![*keyframe.value(), 0.0],
                    };
                    crate::Keyframe::new(
                        keyframe.offset(),
                        values,
                        keyframe.timing_function().cloned(),
                    )
                })
                .collect();
            return AnimationKind::Transform(TransformTrack::Smil {
                kind: TransformKind::Translate,
                keyframes,
            });
        }
    }
    kind
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

fn offsets(count: usize, key_times: Option<&[crate::NormalizedF32]>) -> Vec<crate::NormalizedF32> {
    if let Some(key_times) = key_times.filter(|key_times| key_times.len() == count) {
        return key_times.to_vec();
    }
    if count <= 1 {
        return vec![crate::NormalizedF32::ZERO];
    }
    (0..count)
        .map(|index| crate::NormalizedF32::new_clamped(index as f32 / (count - 1) as f32))
        .collect()
}

fn is_shape_geometry(node: SvgNode, name: &str) -> bool {
    matches!(
        node.tag_name(),
        Some(
            EId::Rect
                | EId::Circle
                | EId::Ellipse
                | EId::Line
                | EId::Polyline
                | EId::Polygon
                | EId::Path
        )
    ) && matches!(
        name,
        "x" | "y"
            | "width"
            | "height"
            | "rx"
            | "ry"
            | "cx"
            | "cy"
            | "r"
            | "x1"
            | "y1"
            | "x2"
            | "y2"
            | "d"
            | "points"
    )
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
