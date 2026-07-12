// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parsing of SMIL `values`/`from`/`to`/`by` forms into typed keyframes.
//!
//! This is the value layer only: it produces typed [`AnimationKind`] keyframe
//! tracks together with the resolved `additive`, `accumulate` and `calcMode`.
//! Timing and interval resolution live elsewhere.

use std::str::FromStr;

use crate::tree::animation::{
    Accumulate, Additive, AnimationKind, AnimationVisibility, CalcMode, Keyframe, Track,
    TransformKind, TransformTrack,
};
use crate::{FillRule, LineCap, LineJoin, NonZeroRect, NormalizedF32, Opacity, StrokeMiterlimit};

/// The parsed result of a SMIL value animation.
#[derive(Clone, Debug)]
pub(crate) struct SmilValues {
    /// The typed keyframe data.
    pub(crate) kind: AnimationKind,
    /// The resolved additive behavior.
    pub(crate) additive: Additive,
    /// The resolved accumulate behavior.
    pub(crate) accumulate: Accumulate,
    /// The resolved calculation mode.
    pub(crate) calc_mode: CalcMode,
}

/// The static underlying value used to resolve `to`-only and `by` forms.
#[derive(Clone, Debug)]
pub(crate) enum BaseValue {
    /// No usable base value.
    None,
    /// An `opacity` or `stop-opacity` base.
    Opacity(Opacity),
    /// A `fill`, `stroke`, or `stop-color` base.
    Color(svgtypes::Color),
    /// A scalar base (stroke width, dash offset, geometry).
    Number(f32),
    /// A `stroke-dasharray` base.
    Numbers(Vec<f32>),
    /// A `stroke-miterlimit` base.
    Miterlimit(StrokeMiterlimit),
    /// A `display` base (`true` when shown).
    Boolean(bool),
    /// A `stroke-linecap` base.
    Linecap(LineCap),
    /// A `stroke-linejoin` base.
    Linejoin(LineJoin),
    /// A `fill-rule` base.
    FillRule(FillRule),
    /// A `visibility` base.
    Visibility(AnimationVisibility),
    /// A stop `offset` base.
    StopOffset(NormalizedF32),
    /// A `viewBox` base.
    Rect(NonZeroRect),
}

impl BaseValue {
    fn opacity(&self) -> Option<Opacity> {
        match self {
            BaseValue::Opacity(v) => Some(*v),
            _ => None,
        }
    }

    fn color(&self) -> Option<svgtypes::Color> {
        match self {
            BaseValue::Color(v) => Some(*v),
            _ => None,
        }
    }

    fn number(&self) -> Option<f32> {
        match self {
            BaseValue::Number(v) => Some(*v),
            _ => None,
        }
    }

    fn numbers(&self) -> Option<Vec<f32>> {
        match self {
            BaseValue::Numbers(v) => Some(v.clone()),
            _ => None,
        }
    }

    fn miterlimit(&self) -> Option<StrokeMiterlimit> {
        match self {
            BaseValue::Miterlimit(v) => Some(*v),
            _ => None,
        }
    }

    fn boolean(&self) -> Option<bool> {
        match self {
            BaseValue::Boolean(v) => Some(*v),
            _ => None,
        }
    }

    fn linecap(&self) -> Option<LineCap> {
        match self {
            BaseValue::Linecap(v) => Some(*v),
            _ => None,
        }
    }

    fn linejoin(&self) -> Option<LineJoin> {
        match self {
            BaseValue::Linejoin(v) => Some(*v),
            _ => None,
        }
    }

    fn fill_rule(&self) -> Option<FillRule> {
        match self {
            BaseValue::FillRule(v) => Some(*v),
            _ => None,
        }
    }

    fn visibility(&self) -> Option<AnimationVisibility> {
        match self {
            BaseValue::Visibility(v) => Some(*v),
            _ => None,
        }
    }

    fn stop_offset(&self) -> Option<NormalizedF32> {
        match self {
            BaseValue::StopOffset(v) => Some(*v),
            _ => None,
        }
    }

    fn rect(&self) -> Option<NonZeroRect> {
        match self {
            BaseValue::Rect(v) => Some(*v),
            _ => None,
        }
    }
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

    match attribute_name {
        "opacity" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                Some(Opacity::ZERO),
                base_value.opacity(),
                |s| warned(parse_opacity(s), s),
                |a, b| Opacity::new_clamped(a.get() + b.get()),
            )?;
            Some(SmilValues {
                kind: AnimationKind::Opacity(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stop-opacity" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                Some(Opacity::ZERO),
                base_value.opacity(),
                |s| warned(parse_opacity(s), s),
                |a, b| Opacity::new_clamped(a.get() + b.get()),
            )?;
            Some(SmilValues {
                kind: AnimationKind::StopOpacity(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "fill" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.color(),
                parse_color_form,
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::Fill(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stroke" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.color(),
                parse_color_form,
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::Stroke(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stop-color" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.color(),
                parse_color_form,
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::StopColor(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stroke-width" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                Some(0.0f32),
                base_value.number(),
                |s| warned(parse_nonneg_number(s), s),
                |a, b| a + b,
            )?;
            Some(SmilValues {
                kind: AnimationKind::StrokeWidth(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stroke-dashoffset" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                Some(0.0f32),
                base_value.number(),
                |s| warned(parse_number(s), s),
                |a, b| a + b,
            )?;
            Some(SmilValues {
                kind: AnimationKind::StrokeDashoffset(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stroke-dasharray" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                None,
                base_value.numbers(),
                |s| warned(parse_number_list(s), s),
                |a, b| {
                    let len = a.len().max(b.len());
                    (0..len)
                        .map(|i| a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0))
                        .collect()
                },
            )?;
            let calc_mode = dasharray_calc_mode(&keyframes, calc_mode);
            Some(SmilValues {
                kind: AnimationKind::StrokeDasharray(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stroke-miterlimit" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                None,
                base_value.miterlimit(),
                |s| warned(parse_miterlimit(s), s),
                |a, b| StrokeMiterlimit::new(a.get() + b.get()),
            )?;
            Some(SmilValues {
                kind: AnimationKind::StrokeMiterlimit(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "stroke-linecap" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.linecap(),
                |s| warned(parse_linecap(s), s),
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::StrokeLinecap(Track::new(keyframes)),
                additive,
                accumulate: resolve_accumulate(accumulate, false),
                calc_mode: CalcMode::Discrete,
            })
        }
        "stroke-linejoin" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.linejoin(),
                |s| warned(parse_linejoin(s), s),
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::StrokeLinejoin(Track::new(keyframes)),
                additive,
                accumulate: resolve_accumulate(accumulate, false),
                calc_mode: CalcMode::Discrete,
            })
        }
        "fill-rule" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.fill_rule(),
                |s| warned(parse_fill_rule(s), s),
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::FillRule(Track::new(keyframes)),
                additive,
                accumulate: resolve_accumulate(accumulate, false),
                calc_mode: CalcMode::Discrete,
            })
        }
        "display" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.boolean(),
                |s| Some(parse_display(s)),
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::Display(Track::new(keyframes)),
                additive,
                accumulate: resolve_accumulate(accumulate, false),
                calc_mode: CalcMode::Discrete,
            })
        }
        "visibility" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                false,
                None,
                base_value.visibility(),
                |s| warned(parse_visibility(s), s),
                |a, _| *a,
            )?;
            Some(SmilValues {
                kind: AnimationKind::Visibility(Track::new(keyframes)),
                additive,
                accumulate: resolve_accumulate(accumulate, false),
                calc_mode: CalcMode::Discrete,
            })
        }
        "offset" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                false,
                true,
                Some(NormalizedF32::ZERO),
                base_value.stop_offset(),
                |s| warned(parse_offset(s), s),
                |a, b| NormalizedF32::new_clamped(a.get() + b.get()),
            )?;
            Some(SmilValues {
                kind: AnimationKind::StopOffset(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "cx" | "cy" | "r" | "rx" | "ry" | "x" | "y" | "x1" | "y1" | "x2" | "y2" | "width"
        | "height" | "fr" | "fx" | "fy" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                true,
                true,
                Some(0.0f32),
                base_value.number(),
                |s| warned_geometry(parse_geometry_number(s), s),
                |a, b| a + b,
            )?;
            Some(SmilValues {
                kind: AnimationKind::GradientGeometry(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
        "viewBox" => {
            let (keyframes, additive) = build_forms(
                &forms,
                key_times,
                additive,
                true,
                true,
                None,
                base_value.rect(),
                |s| warned(parse_rect(s), s),
                add_rects,
            )?;
            Some(SmilValues {
                kind: AnimationKind::ViewBox(Track::new(keyframes)),
                additive,
                accumulate,
                calc_mode,
            })
        }
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
    kind: TransformKind,
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

    let track = TransformTrack::Smil { kind, keyframes };
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

/// The raw string values of the four SMIL value forms.
struct Forms<'a> {
    values: Option<&'a str>,
    from: Option<&'a str>,
    to: Option<&'a str>,
    by: Option<&'a str>,
}

/// Builds a keyframe track from the SMIL value forms and returns the resolved
/// additive behavior.
///
/// * `is_geometry` bakes a bare `by` against `base` instead of a `Sum` delta.
/// * `supports_delta` gates the `by` forms for non-interpolable types.
/// * `sum_zero` is the additive identity used for a bare `by` `Sum` delta.
fn build_forms<T, P, A>(
    forms: &Forms,
    key_times: Option<&[NormalizedF32]>,
    additive: Additive,
    is_geometry: bool,
    supports_delta: bool,
    sum_zero: Option<T>,
    base: Option<T>,
    parse: P,
    delta_add: A,
) -> Option<(Vec<Keyframe<T>>, Additive)>
where
    T: Clone,
    P: Fn(&str) -> Option<T>,
    A: Fn(&T, &T) -> T,
{
    if let Some(values) = forms.values {
        return build_values_list(values, key_times, additive, parse);
    }

    let from = match forms.from {
        Some(s) => Some(parse(s.trim())?),
        None => None,
    };
    let to = match forms.to {
        Some(s) => Some(parse(s.trim())?),
        None => None,
    };
    let by = match forms.by {
        Some(s) => Some(parse(s.trim())?),
        None => None,
    };

    match (from, to, by) {
        (Some(f), Some(t), None) => Some((two_keyframes(f, t), additive)),
        (Some(f), None, Some(b)) => {
            if !supports_delta {
                warn_invalid_value(forms.by.unwrap_or_default().trim());
                return None;
            }
            let end = delta_add(&f, &b);
            Some((two_keyframes(f, end), Additive::Replace))
        }
        (None, Some(t), None) => {
            let base = base?;
            Some((two_keyframes(base, t), Additive::Replace))
        }
        (None, None, Some(b)) => {
            if !supports_delta {
                warn_invalid_value(forms.by.unwrap_or_default().trim());
                return None;
            }
            if is_geometry {
                let base = base?;
                let end = delta_add(&base, &b);
                Some((two_keyframes(base, end), Additive::Replace))
            } else {
                let zero = sum_zero?;
                Some((two_keyframes(zero, b), Additive::Sum))
            }
        }
        (Some(f), None, None) => Some((vec![Keyframe::new(NormalizedF32::ZERO, f, None)], additive)),
        _ => None,
    }
}

/// Builds a keyframe track from a `values` list, dropping invalid entries.
fn build_values_list<T, P>(
    values: &str,
    key_times: Option<&[NormalizedF32]>,
    additive: Additive,
    parse: P,
) -> Option<(Vec<Keyframe<T>>, Additive)>
where
    T: Clone,
    P: Fn(&str) -> Option<T>,
{
    let raw: Vec<&str> = values
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if raw.is_empty() {
        return None;
    }

    let offsets = uniform_offsets(raw.len(), key_times);
    let mut keyframes = Vec::new();
    for (offset, item) in offsets.iter().zip(raw.iter()) {
        if let Some(value) = parse(item) {
            keyframes.push(Keyframe::new(*offset, value, None));
        }
    }

    (!keyframes.is_empty()).then_some((keyframes, additive))
}

/// Builds the two-keyframe track shared by the `from`/`to` and `by` forms.
fn two_keyframes<T: Clone>(start: T, end: T) -> Vec<Keyframe<T>> {
    vec![
        Keyframe::new(NormalizedF32::ZERO, start, None),
        Keyframe::new(NormalizedF32::ONE, end, None),
    ]
}

/// Computes keyframe offsets, honoring `keyTimes` when it matches the count.
fn uniform_offsets(count: usize, key_times: Option<&[NormalizedF32]>) -> Vec<NormalizedF32> {
    if let Some(times) = key_times {
        if times.len() == count {
            return times.to_vec();
        }
    }

    if count <= 1 {
        return vec![NormalizedF32::ZERO];
    }

    (0..count)
        .map(|i| NormalizedF32::new_clamped(i as f32 / (count as f32 - 1.0)))
        .collect()
}

/// Forces discrete stepping when the dash-array keyframes differ in length.
fn dasharray_calc_mode(keyframes: &[Keyframe<Vec<f32>>], calc_mode: CalcMode) -> CalcMode {
    let Some(first) = keyframes.first() else {
        return calc_mode;
    };

    let len = first.value().len();
    if keyframes.iter().any(|k| k.value().len() != len) {
        warn_not_interpolable();
        CalcMode::Discrete
    } else {
        calc_mode
    }
}

/// Drops `Sum` accumulation for types that cannot accumulate.
fn resolve_accumulate(accumulate: Accumulate, accumulatable: bool) -> Accumulate {
    if !accumulatable && matches!(accumulate, Accumulate::Sum) {
        warn_unsupported_accumulate();
        Accumulate::None
    } else {
        accumulate
    }
}

/// Warns and returns `None` when a value failed to parse.
fn warned<T>(value: Option<T>, raw: &str) -> Option<T> {
    if value.is_none() {
        warn_invalid_value(raw);
    }
    value
}

/// Warns and returns `None` when a geometry value failed to parse.
fn warned_geometry<T>(value: Option<T>, raw: &str) -> Option<T> {
    if value.is_none() {
        warn_invalid_geometry_value(raw);
    }
    value
}

fn parse_opacity(value: &str) -> Option<Opacity> {
    let length = svgtypes::Length::from_str(value).ok()?;
    match length.unit {
        svgtypes::LengthUnit::Percent => Some(Opacity::new_clamped(length.number as f32 / 100.0)),
        svgtypes::LengthUnit::None => Some(Opacity::new_clamped(length.number as f32)),
        _ => None,
    }
}

fn parse_offset(value: &str) -> Option<NormalizedF32> {
    parse_opacity(value)
}

fn parse_number(value: &str) -> Option<f32> {
    svgtypes::Number::from_str(value).ok().map(|n| n.0 as f32)
}

fn parse_nonneg_number(value: &str) -> Option<f32> {
    let number = parse_number(value)?;
    (number >= 0.0).then_some(number)
}

fn parse_geometry_number(value: &str) -> Option<f32> {
    svgtypes::Length::from_str(value)
        .ok()
        .map(|l| l.number as f32)
}

fn parse_miterlimit(value: &str) -> Option<StrokeMiterlimit> {
    parse_number(value).map(StrokeMiterlimit::new)
}

fn parse_number_list(value: &str) -> Option<Vec<f32>> {
    let mut list = Vec::new();
    for number in svgtypes::NumberListParser::from(value) {
        list.push(number.ok()? as f32);
    }
    (!list.is_empty()).then_some(list)
}

fn parse_linecap(value: &str) -> Option<LineCap> {
    match value {
        "butt" => Some(LineCap::Butt),
        "round" => Some(LineCap::Round),
        "square" => Some(LineCap::Square),
        _ => None,
    }
}

fn parse_linejoin(value: &str) -> Option<LineJoin> {
    match value {
        "miter" => Some(LineJoin::Miter),
        "miter-clip" => Some(LineJoin::MiterClip),
        "round" => Some(LineJoin::Round),
        "bevel" => Some(LineJoin::Bevel),
        _ => None,
    }
}

fn parse_fill_rule(value: &str) -> Option<FillRule> {
    match value {
        "nonzero" => Some(FillRule::NonZero),
        "evenodd" => Some(FillRule::EvenOdd),
        _ => None,
    }
}

fn parse_display(value: &str) -> bool {
    value != "none"
}

fn parse_visibility(value: &str) -> Option<AnimationVisibility> {
    match value {
        "visible" => Some(AnimationVisibility::Visible),
        "hidden" => Some(AnimationVisibility::Hidden),
        "collapse" => Some(AnimationVisibility::Collapse),
        _ => None,
    }
}

fn parse_rect(value: &str) -> Option<NonZeroRect> {
    let vb = svgtypes::ViewBox::from_str(value).ok()?;
    NonZeroRect::from_xywh(vb.x as f32, vb.y as f32, vb.w as f32, vb.h as f32)
}

/// The outcome of parsing a paint value as a solid color.
enum ColorForm {
    /// A solid color.
    Color(svgtypes::Color),
    /// A `url(#...)` paint reference, which cannot be animated as a color.
    Url,
    /// An unparsable value.
    Invalid,
}

fn parse_paint_color(value: &str) -> ColorForm {
    match svgtypes::Paint::from_str(value) {
        Ok(svgtypes::Paint::Color(color)) => ColorForm::Color(color),
        Ok(svgtypes::Paint::FuncIRI(..)) => ColorForm::Url,
        _ => ColorForm::Invalid,
    }
}

/// Parses a solid color, warning on `url(#...)` and invalid values.
fn parse_color_form(value: &str) -> Option<svgtypes::Color> {
    match parse_paint_color(value) {
        ColorForm::Color(color) => Some(color),
        ColorForm::Url => {
            warn_unsupported_paint(value);
            None
        }
        ColorForm::Invalid => {
            warn_invalid_value(value);
            None
        }
    }
}

/// Adds two rects component-wise, falling back to `a` on a degenerate result.
fn add_rects(a: &NonZeroRect, b: &NonZeroRect) -> NonZeroRect {
    NonZeroRect::from_xywh(
        a.x() + b.x(),
        a.y() + b.y(),
        a.width() + b.width(),
        a.height() + b.height(),
    )
    .unwrap_or(*a)
}

fn warn_unsupported_attribute(name: &str) {
    log::warn!("Unsupported animation attribute: '{}'.", name);
}

fn warn_unsupported_paint(value: &str) {
    log::warn!("Unsupported paint value: '{}'.", value);
}

fn warn_not_interpolable() {
    log::warn!("Animation values are not interpolable; using discrete interpolation.");
}

fn warn_invalid_value(value: &str) {
    log::warn!("Invalid animation value: '{}'.", value);
}

fn warn_invalid_geometry_value(value: &str) {
    log::warn!("Invalid geometry animation value: '{}'.", value);
}

fn warn_last_additive_geometry() {
    log::warn!("Only the last additive geometry animation is used.");
}

fn warn_unsupported_accumulate() {
    log::warn!("Unsupported accumulate value; ignoring.");
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLACE: Additive = Additive::Replace;
    const NONE: Accumulate = Accumulate::None;
    const LINEAR: CalcMode = CalcMode::Linear;

    fn color(value: &str) -> svgtypes::Color {
        svgtypes::Color::from_str(value).unwrap()
    }

    #[test]
    fn opacity_values_list() {
        let result = parse_smil_values(
            "opacity",
            Some("0;0.5;1"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match result.kind {
            AnimationKind::Opacity(track) => {
                assert_eq!(track.keyframes().len(), 3);
                assert_eq!(track.keyframes()[0].value().get(), 0.0);
                assert_eq!(track.keyframes()[1].value().get(), 0.5);
                assert_eq!(track.keyframes()[2].value().get(), 1.0);
            }
            other => panic!("expected opacity, got {other:?}"),
        }
    }

    #[test]
    fn fill_from_to() {
        let result = parse_smil_values(
            "fill",
            None,
            Some("red"),
            Some("blue"),
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match result.kind {
            AnimationKind::Fill(track) => {
                assert_eq!(track.keyframes().len(), 2);
                assert_eq!(*track.keyframes()[0].value(), color("red"));
                assert_eq!(*track.keyframes()[1].value(), color("blue"));
            }
            other => panic!("expected fill, got {other:?}"),
        }
    }

    #[test]
    fn url_paint_is_dropped() {
        let result = parse_smil_values(
            "fill",
            None,
            None,
            Some("url(#g)"),
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn dasharray_length_mismatch_is_discrete() {
        let result = parse_smil_values(
            "stroke-dasharray",
            Some("1 2;3 4 5"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::StrokeDasharray(track) => assert_eq!(track.keyframes().len(), 2),
            other => panic!("expected dasharray, got {other:?}"),
        }
        assert!(matches!(result.calc_mode, CalcMode::Discrete));
    }

    #[test]
    fn viewbox_values_list() {
        let result = parse_smil_values(
            "viewBox",
            Some("0 0 100 100;10 10 200 200"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::ViewBox(track) => {
                assert_eq!(track.keyframes().len(), 2);
                assert_eq!(track.keyframes()[0].value().width(), 100.0);
                assert_eq!(track.keyframes()[1].value().width(), 200.0);
            }
            other => panic!("expected viewBox, got {other:?}"),
        }
    }

    #[test]
    fn to_only_uses_base() {
        let result = parse_smil_values(
            "stroke-width",
            None,
            None,
            Some("20"),
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Number(10.0),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::StrokeWidth(track) => {
                assert_eq!(track.keyframes().len(), 2);
                assert_eq!(*track.keyframes()[0].value(), 10.0);
                assert_eq!(*track.keyframes()[1].value(), 20.0);
            }
            other => panic!("expected stroke-width, got {other:?}"),
        }
        assert!(matches!(result.additive, Additive::Replace));
    }

    #[test]
    fn from_by_bakes_delta() {
        // Input additive is `Sum`, but `from`/`by` forces `Replace`.
        let result = parse_smil_values(
            "stroke-width",
            None,
            Some("10"),
            None,
            Some("5"),
            Additive::Sum,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::StrokeWidth(track) => {
                assert_eq!(*track.keyframes()[0].value(), 10.0);
                assert_eq!(*track.keyframes()[1].value(), 15.0);
            }
            other => panic!("expected stroke-width, got {other:?}"),
        }
        assert!(matches!(result.additive, Additive::Replace));
    }

    #[test]
    fn bare_by_non_geometry_is_sum() {
        // Input additive is `Replace`, but a bare `by` forces `Sum`.
        let result = parse_smil_values(
            "stroke-width",
            None,
            None,
            None,
            Some("5"),
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::StrokeWidth(track) => {
                assert_eq!(*track.keyframes()[0].value(), 0.0);
                assert_eq!(*track.keyframes()[1].value(), 5.0);
            }
            other => panic!("expected stroke-width, got {other:?}"),
        }
        assert!(matches!(result.additive, Additive::Sum));
    }

    #[test]
    fn bare_by_geometry_bakes_base() {
        let result = parse_smil_values(
            "cx",
            None,
            None,
            None,
            Some("50"),
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Number(100.0),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::GradientGeometry(track) => {
                assert_eq!(*track.keyframes()[0].value(), 100.0);
                assert_eq!(*track.keyframes()[1].value(), 150.0);
            }
            other => panic!("expected geometry, got {other:?}"),
        }
        assert!(matches!(result.additive, Additive::Replace));
    }

    #[test]
    fn invalid_value_is_dropped() {
        let result = parse_smil_values(
            "fill",
            Some("red;notacolor;blue"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Fill(track) => {
                assert_eq!(track.keyframes().len(), 2);
                assert_eq!(*track.keyframes()[0].value(), color("red"));
                assert_eq!(*track.keyframes()[1].value(), color("blue"));
            }
            other => panic!("expected fill, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_attribute_is_none() {
        let result = parse_smil_values(
            "font-size",
            None,
            Some("10"),
            Some("20"),
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn set_single_value_is_discrete() {
        // `<set>` is modeled as a single-item `values` list with a discrete mode.
        let result = parse_smil_values(
            "opacity",
            Some("0.5"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            CalcMode::Discrete,
            None,
            &BaseValue::None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Opacity(track) => {
                assert_eq!(track.keyframes().len(), 1);
                assert_eq!(track.keyframes()[0].value().get(), 0.5);
                assert_eq!(track.keyframes()[0].offset().get(), 0.0);
            }
            other => panic!("expected opacity, got {other:?}"),
        }
        assert!(matches!(result.calc_mode, CalcMode::Discrete));
    }

    #[test]
    fn transform_translate_from_to() {
        let result = parse_smil_transform_values(
            TransformKind::Translate,
            false,
            None,
            Some("0 0"),
            Some("10 20"),
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            None,
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Transform(TransformTrack::Smil { kind, keyframes }) => {
                assert!(matches!(kind, TransformKind::Translate));
                assert_eq!(keyframes.len(), 2);
                assert_eq!(keyframes[1].value(), &vec![10.0, 20.0]);
            }
            other => panic!("expected transform, got {other:?}"),
        }
    }

    #[test]
    fn base_value_extractors() {
        assert!(BaseValue::None.number().is_none());
        assert_eq!(
            BaseValue::Opacity(Opacity::new_clamped(0.5))
                .opacity()
                .unwrap()
                .get(),
            0.5
        );
        assert_eq!(BaseValue::Color(color("red")).color().unwrap(), color("red"));
        assert_eq!(BaseValue::Number(3.0).number().unwrap(), 3.0);
        assert_eq!(
            BaseValue::Numbers(vec![1.0, 2.0]).numbers().unwrap(),
            vec![1.0, 2.0]
        );
        assert_eq!(
            BaseValue::Miterlimit(StrokeMiterlimit::new(4.0))
                .miterlimit()
                .unwrap()
                .get(),
            4.0
        );
        assert!(BaseValue::Boolean(true).boolean().unwrap());
        assert!(matches!(
            BaseValue::Linecap(LineCap::Round).linecap().unwrap(),
            LineCap::Round
        ));
        assert!(matches!(
            BaseValue::Linejoin(LineJoin::Bevel).linejoin().unwrap(),
            LineJoin::Bevel
        ));
        assert!(matches!(
            BaseValue::FillRule(FillRule::EvenOdd).fill_rule().unwrap(),
            FillRule::EvenOdd
        ));
        assert!(matches!(
            BaseValue::Visibility(AnimationVisibility::Hidden)
                .visibility()
                .unwrap(),
            AnimationVisibility::Hidden
        ));
        assert_eq!(
            BaseValue::StopOffset(NormalizedF32::new_clamped(0.25))
                .stop_offset()
                .unwrap()
                .get(),
            0.25
        );
        let rect = NonZeroRect::from_xywh(0.0, 0.0, 10.0, 10.0).unwrap();
        assert_eq!(BaseValue::Rect(rect).rect().unwrap().width(), 10.0);
    }

    #[test]
    fn additive_geometry_warning_literal() {
        // Exercises the cross-animation helper owned by this module.
        warn_last_additive_geometry();
    }
}
