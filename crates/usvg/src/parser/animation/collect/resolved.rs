// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Target-specific resolution for scalar SMIL geometry tracks.

use std::str::FromStr;

use svgtypes::{Length, LengthUnit as Unit};

use super::super::values::forms::Forms;
use super::super::values::{SmilValues, parse_resolved_geometry_values};
use crate::Units;
use crate::parser::converter;
use crate::parser::paint_server::{convert_units, resolve_number};
use crate::parser::svgtree::{AId, EId, SvgNode};
use crate::parser::units;
use crate::tree::animation::{
    Accumulate, Additive, AnimationKind, Easing, GradientGeometry, GradientGeometryComponent,
};

pub(super) fn is_image_geometry(target: SvgNode, attribute_name: &str) -> bool {
    target.tag_name() == Some(EId::Image)
        && matches!(attribute_name, "x" | "y" | "width" | "height")
}

pub(super) fn is_gradient_geometry(target: SvgNode, attribute_name: &str) -> bool {
    matches!(
        target.tag_name(),
        Some(EId::LinearGradient | EId::RadialGradient)
    ) && GradientGeometryComponent::from_attribute_name(attribute_name).is_some()
}

pub(super) fn parse_image_geometry_animation(
    target: SvgNode,
    node: SvgNode,
    attribute_name: &str,
    is_set: bool,
    additive: Additive,
    accumulate: Accumulate,
    easing: &Easing,
    state: &converter::State,
) -> Option<SmilValues> {
    let aid = AId::from_str(attribute_name)?;
    let values = parse_resolved_geometry_values(
        &forms(node, is_set),
        easing.key_times(),
        additive,
        accumulate,
        easing.calc_mode(),
        Some(target.convert_user_length(aid, state, Length::zero())),
        |value| resolve_user_length(value, target, aid, state),
    )?;
    let AnimationKind::Geometry(track) = values.kind else {
        return None;
    };
    let kind = match attribute_name {
        "x" => AnimationKind::ImageX(track),
        "y" => AnimationKind::ImageY(track),
        "width" => AnimationKind::ImageWidth(track),
        "height" => AnimationKind::ImageHeight(track),
        _ => return None,
    };
    Some(SmilValues { kind, ..values })
}

pub(super) fn parse_gradient_geometry_animation(
    target: SvgNode,
    node: SvgNode,
    attribute_name: &str,
    is_set: bool,
    additive: Additive,
    accumulate: Accumulate,
    easing: &Easing,
    state: &converter::State,
) -> Option<SmilValues> {
    let aid = AId::from_str(attribute_name)?;
    let component = GradientGeometryComponent::from_attribute_name(attribute_name)?;
    let units = convert_units(target, AId::GradientUnits, Units::ObjectBoundingBox);
    let values = parse_resolved_geometry_values(
        &forms(node, is_set),
        easing.key_times(),
        additive,
        accumulate,
        easing.calc_mode(),
        gradient_base_value(target, component, units, state),
        |value| resolve_gradient_length(value, target, aid, units, state),
    )?;
    let AnimationKind::Geometry(track) = values.kind else {
        return None;
    };
    Some(SmilValues {
        kind: AnimationKind::GradientGeometry(GradientGeometry::new(component, track)),
        ..values
    })
}

fn forms<'a, 'input: 'a>(node: SvgNode<'a, 'input>, is_set: bool) -> Forms<'a> {
    Forms {
        values: if is_set {
            node.attribute(AId::To)
                .or_else(|| node.attribute(AId::Values))
        } else {
            node.attribute(AId::Values)
        },
        from: (!is_set).then(|| node.attribute(AId::From)).flatten(),
        to: (!is_set).then(|| node.attribute(AId::To)).flatten(),
        by: (!is_set).then(|| node.attribute(AId::By)).flatten(),
    }
}

fn resolve_user_length(
    value: &str,
    target: SvgNode,
    aid: AId,
    state: &converter::State,
) -> Option<f32> {
    let length = Length::from_str(value).ok()?;
    Some(units::convert_user_length(length, target, aid, state))
}

fn resolve_gradient_length(
    value: &str,
    target: SvgNode,
    aid: AId,
    units: Units,
    state: &converter::State,
) -> Option<f32> {
    let length = Length::from_str(value).ok()?;
    Some(units::convert_length(length, target, aid, units, state))
}

fn gradient_base_value(
    target: SvgNode,
    component: GradientGeometryComponent,
    units: Units,
    state: &converter::State,
) -> Option<f32> {
    let value = match component {
        GradientGeometryComponent::LinearX1 => {
            resolve_number(target, AId::X1, units, state, Length::zero())
        }
        GradientGeometryComponent::LinearY1 => {
            resolve_number(target, AId::Y1, units, state, Length::zero())
        }
        GradientGeometryComponent::LinearX2 => resolve_number(
            target,
            AId::X2,
            units,
            state,
            Length::new(100.0, Unit::Percent),
        ),
        GradientGeometryComponent::LinearY2 => {
            resolve_number(target, AId::Y2, units, state, Length::zero())
        }
        GradientGeometryComponent::RadialCx => resolve_number(
            target,
            AId::Cx,
            units,
            state,
            Length::new(50.0, Unit::Percent),
        ),
        GradientGeometryComponent::RadialCy => resolve_number(
            target,
            AId::Cy,
            units,
            state,
            Length::new(50.0, Unit::Percent),
        ),
        GradientGeometryComponent::RadialR => resolve_number(
            target,
            AId::R,
            units,
            state,
            Length::new(50.0, Unit::Percent),
        ),
        GradientGeometryComponent::RadialFx => {
            let cx =
                gradient_base_value(target, GradientGeometryComponent::RadialCx, units, state)?;
            resolve_number(
                target,
                AId::Fx,
                units,
                state,
                Length::new_number(f64::from(cx)),
            )
        }
        GradientGeometryComponent::RadialFy => {
            let cy =
                gradient_base_value(target, GradientGeometryComponent::RadialCy, units, state)?;
            resolve_number(
                target,
                AId::Fy,
                units,
                state,
                Length::new_number(f64::from(cy)),
            )
        }
        GradientGeometryComponent::RadialFr => {
            resolve_number(target, AId::Fr, units, state, Length::zero())
        }
    };
    Some(value)
}
