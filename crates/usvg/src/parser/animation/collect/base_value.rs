// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use svgtypes::Length;

use super::super::values::BaseValue;
use crate::parser::converter;
use crate::parser::svgtree::{AId, SvgNode};
use crate::tree::animation::AnimationVisibility;
use crate::{Opacity, StrokeMiterlimit, Visibility};

pub(super) fn base_value(node: SvgNode, name: &str, state: &converter::State) -> BaseValue {
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
        "display" => BaseValue::Boolean(
            node.parent()
                .and_then(|parent| parent.find_attribute::<&str>(AId::Display))
                != Some("none"),
        ),
        "visibility" => BaseValue::Visibility(
            match node
                .parent()
                .and_then(|parent| parent.find_attribute(AId::Visibility))
                .unwrap_or_default()
            {
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
