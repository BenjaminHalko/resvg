// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::too_many_arguments)]

use crate::tree::animation::{
    Accumulate, Additive, AnimationKind, CalcMode, Keyframe, Track, TransformFunction,
};
use crate::NormalizedF32;

use super::forms::{build_forms, Forms};
use super::stroke::parse_number_list;
use super::{geometry, opacity, paint, presentation, stroke, BaseValue, SmilValues};

pub(super) struct AttributeContext<'a> {
    pub(super) forms: &'a Forms<'a>,
    pub(super) key_times: Option<&'a [NormalizedF32]>,
    pub(super) additive: Additive,
    pub(super) accumulate: Accumulate,
    pub(super) calc_mode: CalcMode,
    pub(super) base_value: &'a BaseValue,
}

#[derive(Clone, Copy)]
pub(crate) enum SmilTransformType {
    Translate,
    Scale,
    Rotate,
    SkewX,
    SkewY,
}

/// Parses a SMIL `<animate>`/`<set>`/`<animateColor>` value animation.
///
/// Returns `None` when the attribute is unsupported or every value is invalid.
/// `<set>` is represented by the caller as a single-item `values` list with a
/// `discrete` `calc_mode`.
pub(crate) fn parse_smil_values(
    attribute_name: &str,
    values_str: Option<&str>,
    from_str: Option<&str>,
    to_str: Option<&str>,
    by_str: Option<&str>,
    additive: Additive,
    accumulate: Accumulate,
    calc_mode: CalcMode,
    key_times: Option<&[NormalizedF32]>,
    base_value: &BaseValue,
) -> Option<SmilValues> {
    let forms = Forms {
        values: values_str,
        from: from_str,
        to: to_str,
        by: by_str,
    };

    let context = AttributeContext {
        forms: &forms,
        key_times,
        additive,
        accumulate,
        calc_mode,
        base_value,
    };

    match attribute_name {
        "opacity" => opacity::parse_opacity_attribute(context),
        "stop-opacity" => opacity::parse_stop_opacity_attribute(context),
        "fill" => paint::parse_fill_attribute(context),
        "stroke" => paint::parse_stroke_attribute(context),
        "stop-color" => paint::parse_stop_color_attribute(context),
        "stroke-width" => stroke::parse_stroke_width_attribute(context),
        "stroke-dashoffset" => stroke::parse_stroke_dashoffset_attribute(context),
        "stroke-dasharray" => stroke::parse_stroke_dasharray_attribute(context),
        "stroke-miterlimit" => stroke::parse_stroke_miterlimit_attribute(context),
        "stroke-linecap" => presentation::parse_stroke_linecap_attribute(context),
        "stroke-linejoin" => presentation::parse_stroke_linejoin_attribute(context),
        "fill-rule" => presentation::parse_fill_rule_attribute(context),
        "display" => presentation::parse_display_attribute(context),
        "visibility" => presentation::parse_visibility_attribute(context),
        "offset" => geometry::parse_offset_attribute(context),
        "cx" | "cy" | "r" | "rx" | "ry" | "x" | "y" | "x1" | "y1" | "x2" | "y2" | "width"
        | "height" | "fr" | "fx" | "fy" => geometry::parse_geometry_attribute(context),
        "viewBox" => geometry::parse_view_box_attribute(context),
        // Transforms carry a `type` and are routed through `parse_smil_transform_values`.
        "transform" | "gradientTransform" => None,
        _ => {
            warn_unsupported_attribute(attribute_name);
            None
        }
    }
}

/// Parses an `<animateTransform>` value animation for a given transform `kind`.
///
/// `gradient` selects between the `transform` and `gradientTransform` targets.
pub(crate) fn parse_smil_transform_values(
    kind: SmilTransformType,
    gradient: bool,
    values_str: Option<&str>,
    from_str: Option<&str>,
    to_str: Option<&str>,
    by_str: Option<&str>,
    additive: Additive,
    accumulate: Accumulate,
    calc_mode: CalcMode,
    key_times: Option<&[NormalizedF32]>,
    base_params: Option<&[f32]>,
) -> Option<SmilValues> {
    let forms = Forms {
        values: values_str,
        from: from_str,
        to: to_str,
        by: by_str,
    };

    let (keyframes, additive) = build_forms(
        &forms,
        key_times,
        additive,
        false,
        true,
        None,
        base_params.map(<[f32]>::to_vec),
        |s| warned(parse_number_list(s), s),
        |a, b| {
            let len = a.len().max(b.len());
            (0..len)
                .map(|i| a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0))
                .collect()
        },
    )?;

    let track = lower_smil_transform(kind, keyframes);
    let kind = if gradient {
        AnimationKind::GradientTransform(track)
    } else {
        AnimationKind::Transform(track)
    };

    Some(SmilValues {
        kind,
        additive,
        accumulate,
        calc_mode,
    })
}

fn lower_smil_transform(
    kind: SmilTransformType,
    keyframes: Vec<Keyframe<Vec<f32>>>,
) -> Track<Vec<TransformFunction>> {
    let all_rotate_centers_default = keyframes.iter().all(|keyframe| {
        parameter(keyframe.value(), 1, 0.0) == 0.0 && parameter(keyframe.value(), 2, 0.0) == 0.0
    });
    Track::new(
        keyframes
            .into_iter()
            .map(|keyframe| {
                let values = keyframe.value();
                let functions = match kind {
                    SmilTransformType::Translate => vec![TransformFunction::Translate(
                        parameter(values, 0, 0.0),
                        parameter(values, 1, 0.0),
                    )],
                    SmilTransformType::Scale => {
                        let sx = parameter(values, 0, 1.0);
                        vec![TransformFunction::Scale(sx, parameter(values, 1, sx))]
                    }
                    SmilTransformType::SkewX => {
                        vec![TransformFunction::SkewX(parameter(values, 0, 0.0))]
                    }
                    SmilTransformType::SkewY => {
                        vec![TransformFunction::SkewY(parameter(values, 0, 0.0))]
                    }
                    SmilTransformType::Rotate if all_rotate_centers_default => {
                        vec![TransformFunction::Rotate(parameter(values, 0, 0.0))]
                    }
                    SmilTransformType::Rotate => {
                        let angle = parameter(values, 0, 0.0);
                        let cx = parameter(values, 1, 0.0);
                        let cy = parameter(values, 2, 0.0);
                        vec![
                            TransformFunction::Translate(cx, cy),
                            TransformFunction::Rotate(angle),
                            TransformFunction::Translate(-cx, -cy),
                        ]
                    }
                };
                Keyframe::new(
                    keyframe.offset(),
                    functions,
                    keyframe.timing_function().cloned(),
                )
            })
            .collect(),
    )
}

fn parameter(values: &[f32], index: usize, default: f32) -> f32 {
    values.get(index).copied().unwrap_or(default)
}

/// Warns and returns `None` when a value failed to parse.
pub(super) fn warned<T>(value: Option<T>, raw: &str) -> Option<T> {
    if value.is_none() {
        warn_invalid_value(raw);
    }
    value
}

/// Warns and returns `None` when a geometry value failed to parse.
pub(super) fn warned_geometry<T>(value: Option<T>, raw: &str) -> Option<T> {
    if value.is_none() {
        warn_invalid_geometry_value(raw);
    }
    value
}

fn warn_unsupported_attribute(name: &str) {
    log::warn!("Unsupported animation attribute: '{}'.", name);
}

pub(super) fn warn_unsupported_paint(value: &str) {
    log::warn!("Unsupported paint value: '{}'.", value);
}

pub(super) fn warn_not_interpolable() {
    log::warn!("Animation values are not interpolable; using discrete interpolation.");
}

pub(super) fn warn_invalid_value(value: &str) {
    log::warn!("Invalid animation value: '{}'.", value);
}

pub(super) fn warn_invalid_geometry_value(value: &str) {
    log::warn!("Invalid geometry animation value: '{}'.", value);
}

pub(super) fn warn_unsupported_accumulate() {
    log::warn!("Unsupported accumulate value; ignoring.");
}
