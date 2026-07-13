// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Conversion of animated gradients that preserves every source stop.
//!
//! An animated gradient bypasses the destructive static stop conversion in
//! `paint_server`: it keeps all source stops with their unmodified offsets,
//! attaches the gradient- and stop-level tracks inside a `GradientAnimation`,
//! and synthesizes a two-stop carrier instead of collapsing to a solid color.
//! Geometry keyframes stay in the gradient's native scalar space; the
//! objectBoundingBox conversion is applied later through `base.transform`.

use std::str::FromStr;
use std::sync::Arc;

use svgtypes::{Length, LengthUnit as Unit};

use super::collect::collect_node_animations;
use crate::parser::converter::{self, SvgColorExt};
use crate::parser::paint_server::{
    convert_spread_method, convert_units, find_gradient_with_stops, radial_focal_is_omitted,
    resolve_number, ServerOrColor,
};
use crate::parser::svgtree::{AId, EId, SvgNode};
use crate::tree::animation::{GradientAnimation, SourceStop};
use crate::{
    BaseGradient, Color, IsValidLength, LinearGradient, NonEmptyString, Opacity, Paint,
    PositiveF32, RadialGradient, Stop, StopOffset, Units,
};

/// Builds an animated gradient, bypassing the static stop conversion.
///
/// Returns `None` when `node` is not an animated gradient, so the caller
/// continues with the static path. Returns `Some(result)` when the gradient
/// carries a gradient- or stop-level animation, where `result` is the built
/// paint server (or `None` if it could not be built).
pub(crate) fn preserve_animated_gradient(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<Option<ServerOrColor>> {
    let tag = node.tag_name()?;
    if !matches!(tag, EId::LinearGradient | EId::RadialGradient) {
        return None;
    }

    let (source_stops, mut stops) = collect_source_stops(node, state, cache);
    if source_stops.is_empty() {
        return None;
    }

    let animations = collect_node_animations(node, state, cache);
    let stop_animated = source_stops
        .iter()
        .any(|stop| !stop.animations().is_empty());
    if animations.is_empty() && !stop_animated {
        return None;
    }

    let id = NonEmptyString::new(node.element_id().to_string())?;
    let units = convert_units(node, AId::GradientUnits, Units::ObjectBoundingBox);
    let transform = node.resolve_transform(AId::GradientTransform, state);
    let spread_method = convert_spread_method(node);

    let mut source_indices: Vec<usize> = (0..stops.len()).collect();
    if stops.len() < 2 {
        let mut carrier = stops[0];
        carrier.offset = StopOffset::new_clamped(1.0);
        stops.push(carrier);
        source_indices.push(source_indices[0]);
    }

    let underlying_r = if tag == EId::RadialGradient {
        let r = resolve_number(node, AId::R, units, state, Length::new(50.0, Unit::Percent));
        (!r.is_valid_length()).then_some(r)
    } else {
        None
    };
    let (focal_x_is_omitted, focal_y_is_omitted) = if tag == EId::RadialGradient {
        (
            radial_focal_is_omitted(node, AId::Fx),
            radial_focal_is_omitted(node, AId::Fy),
        )
    } else {
        (false, false)
    };

    let base = BaseGradient {
        id,
        units,
        transform,
        spread_method,
        stops,
        animation: Some(Box::new(GradientAnimation::new(
            animations,
            underlying_r,
            focal_x_is_omitted,
            focal_y_is_omitted,
            source_stops,
            source_indices,
        ))),
    };

    let paint = if tag == EId::LinearGradient {
        Paint::LinearGradient(Arc::new(build_linear(node, state, units, base)))
    } else {
        Paint::RadialGradient(Arc::new(build_radial(node, state, units, base)))
    };

    Some(Some(ServerOrColor::Server(paint)))
}

/// Resolves the linear gradient geometry, mirroring `convert_linear`.
fn build_linear(
    node: SvgNode,
    state: &converter::State,
    units: Units,
    base: BaseGradient,
) -> LinearGradient {
    LinearGradient {
        x1: resolve_number(node, AId::X1, units, state, Length::zero()),
        y1: resolve_number(node, AId::Y1, units, state, Length::zero()),
        x2: resolve_number(
            node,
            AId::X2,
            units,
            state,
            Length::new(100.0, Unit::Percent),
        ),
        y2: resolve_number(node, AId::Y2, units, state, Length::zero()),
        base,
    }
}

/// Resolves the radial gradient geometry, mirroring `convert_radial`.
///
/// A non-positive static `r` yields a placeholder carrier radius; the true
/// static radius rides in `GradientAnimation::underlying_r`.
fn build_radial(
    node: SvgNode,
    state: &converter::State,
    units: Units,
    base: BaseGradient,
) -> RadialGradient {
    let r = resolve_number(node, AId::R, units, state, Length::new(50.0, Unit::Percent));
    let cx = resolve_number(
        node,
        AId::Cx,
        units,
        state,
        Length::new(50.0, Unit::Percent),
    );
    let cy = resolve_number(
        node,
        AId::Cy,
        units,
        state,
        Length::new(50.0, Unit::Percent),
    );
    RadialGradient {
        cx,
        cy,
        r: PositiveF32::new(r).unwrap_or_else(|| PositiveF32::new(1.0).unwrap()),
        fx: resolve_number(node, AId::Fx, units, state, Length::new_number(cx as f64)),
        fy: resolve_number(node, AId::Fy, units, state, Length::new_number(cy as f64)),
        fr: PositiveF32::new(resolve_number(node, AId::Fr, units, state, Length::zero()))
            .unwrap_or(PositiveF32::ZERO),
        base,
    }
}

/// Collects every source stop and its per-stop tracks without removing or
/// shifting offsets, alongside the matching converted `Stop` list.
fn collect_source_stops(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> (Vec<SourceStop>, Vec<Stop>) {
    let mut source_stops = Vec::new();
    let mut stops = Vec::new();
    let Some(grad) = find_gradient_with_stops(node) else {
        return (source_stops, stops);
    };

    let mut prev_offset = Length::zero();
    for stop in grad.children() {
        if stop.tag_name() != Some(EId::Stop) {
            continue;
        }

        let offset = stop.attribute(AId::Offset).unwrap_or(prev_offset);
        let offset = match offset.unit {
            Unit::None => offset.number,
            Unit::Percent => offset.number / 100.0,
            _ => prev_offset.number,
        };
        prev_offset = Length::new_number(offset);
        let offset = StopOffset::new_clamped(crate::f32_bound(0.0, offset as f32, 1.0));

        let (color, opacity) = stop_color(stop);
        let stop_opacity = stop
            .attribute::<Opacity>(AId::StopOpacity)
            .unwrap_or(Opacity::ONE);

        source_stops.push(SourceStop::new(collect_node_animations(stop, state, cache)));
        stops.push(Stop {
            offset,
            color,
            opacity: opacity * stop_opacity,
        });
    }

    (source_stops, stops)
}

/// Resolves a stop's color and opacity, mirroring `convert_stops`.
fn stop_color(stop: SvgNode) -> (Color, Opacity) {
    match stop.attribute(AId::StopColor) {
        Some("currentColor") => stop
            .find_attribute(AId::Color)
            .unwrap_or_else(svgtypes::Color::black),
        Some(value) => match svgtypes::Color::from_str(value) {
            Ok(color) => color,
            Err(_) => {
                log::warn!("Failed to parse stop-color value: '{}'.", value);
                svgtypes::Color::black()
            }
        },
        _ => svgtypes::Color::black(),
    }
    .split_alpha()
}
