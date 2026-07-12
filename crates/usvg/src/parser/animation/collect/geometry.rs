// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use svgtypes::Length;

use super::super::geom::{
    ShapeGeometry, bake_geometry_animation, bake_geometry_animation_with_sum_base,
};
use super::super::values::{SmilValues, parse_smil_values};
use super::base_value::base_value;
use crate::parser::converter;
use crate::parser::svgtree::{AId, EId, SvgNode};
use crate::tree::animation::{Accumulate, Additive, AnimationKind, Easing};

pub(super) fn parse_geometry_animation(
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
    let (mut values, mut offsets, raw_values) = if matches!(attribute_name, "d" | "points") {
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
    let sum_over_base =
        !matches!(attribute_name, "d" | "points") && matches!(additive, Additive::Sum);
    if sum_over_base {
        let base = geometry.attribute(attribute_name)?;
        for value in &mut values {
            *value += base;
        }
    }
    if !is_set
        && matches!(easing.calc_mode(), crate::CalcMode::Discrete)
        && easing.key_times().is_none()
        && node.has_attribute(AId::From)
        && node.has_attribute(AId::To)
        && values.len() == 2
    {
        offsets[1] = crate::NormalizedF32::new_clamped(0.5);
    }
    let key_timing_fns = vec![None; offsets.len()];
    let bake = (if sum_over_base {
        bake_geometry_animation_with_sum_base
    } else {
        bake_geometry_animation
    })(
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
        additive: if sum_over_base {
            Additive::Replace
        } else {
            additive
        },
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
    let rx_is_implicit = !node.has_attribute(AId::Rx);
    let ry_is_implicit = !node.has_attribute(AId::Ry);
    let rx = node.convert_user_length(AId::Rx, state, Length::zero());
    let ry = node.convert_user_length(AId::Ry, state, Length::zero());
    ShapeGeometry {
        x: node.convert_user_length(AId::X, state, Length::zero()),
        y: node.convert_user_length(AId::Y, state, Length::zero()),
        width: node.convert_user_length(AId::Width, state, Length::zero()),
        height: node.convert_user_length(AId::Height, state, Length::zero()),
        rx: if rx_is_implicit { ry } else { rx },
        ry: if ry_is_implicit { rx } else { ry },
        cx: node.convert_user_length(AId::Cx, state, Length::zero()),
        cy: node.convert_user_length(AId::Cy, state, Length::zero()),
        r: node.convert_user_length(AId::R, state, Length::zero()),
        x1: node.convert_user_length(AId::X1, state, Length::zero()),
        y1: node.convert_user_length(AId::Y1, state, Length::zero()),
        x2: node.convert_user_length(AId::X2, state, Length::zero()),
        y2: node.convert_user_length(AId::Y2, state, Length::zero()),
        #[cfg(feature = "animation")]
        rx_is_implicit,
        #[cfg(feature = "animation")]
        ry_is_implicit,
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

pub(super) fn is_shape_geometry(node: SvgNode, name: &str) -> bool {
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
