// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::NormalizedF32;
use crate::parser::svgtree::{AId, Document, EId, SvgNode};
use crate::tree::animation::{
    Accumulate, Additive, Animation, AnimationKind, AnimationSource, CalcMode, CssOrigin, Easing,
    Keyframe, Timing, TimingFunction, Track,
};

use self::metadata::{
    bake_timing, cycle, is_paused, longhand_list, parse_direction, parse_fill_mode,
    parse_iterations, parse_time, read_transform_origin, split_list,
};
use self::path::build_css_path_track;
use self::timing::parse_timing_function;
use self::transform::parse_transform_functions;
use self::values::{
    parse_css_color, parse_css_opacity, parse_css_stroke_dashoffset, parse_css_stroke_width,
};
use super::keyframes::KeyframesRule;

mod metadata;
mod path;
mod timing;
mod transform;
mod values;

/// Builds the CSS animations attached to `node` via its `animation-name`.
///
/// Each `animation-name` is matched against the document's `@keyframes` rules
/// and expanded into one [`Animation`] per animated CSS property. Unsupported
/// properties, unknown keyframes names and CSS variables are dropped with a
/// warning.
///
/// A rule that omits the `0%`/`100%` keyframes keeps only its explicit
/// keyframes; the sampler supplies the underlying value at the missing edges.
pub(crate) fn build_css_animations<'a, 'input>(
    node: SvgNode<'a, 'input>,
    doc: &'a Document<'input>,
    state: &crate::parser::converter::State,
) -> Vec<Arc<Animation>> {
    let Some(names) = node.attribute::<&str>(AId::AnimationName) else {
        return Vec::new();
    };

    let names = split_list(names);
    let durations = longhand_list(node, AId::AnimationDuration);
    let delays = longhand_list(node, AId::AnimationDelay);
    let iteration_counts = longhand_list(node, AId::AnimationIterationCount);
    let directions = longhand_list(node, AId::AnimationDirection);
    let fill_modes = longhand_list(node, AId::AnimationFillMode);
    let timing_functions = longhand_list(node, AId::AnimationTimingFunction);
    let play_states = longhand_list(node, AId::AnimationPlayState);

    let is_stop = node.tag_name() == Some(EId::Stop);
    let origin = read_transform_origin(node, state);

    let mut animations = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let name = name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            continue;
        }

        let Some(rule) = doc
            .keyframes()
            .iter()
            .find(|rule| rule.name.as_str() == name)
        else {
            log::warn!("Unknown keyframes name: '{}'.", name);
            continue;
        };

        let timing = bake_timing(
            parse_time(cycle(&durations, index).unwrap_or("0s")).unwrap_or(0.0),
            parse_time(cycle(&delays, index).unwrap_or("0s")).unwrap_or(0.0),
            parse_iterations(cycle(&iteration_counts, index).unwrap_or("1")),
            parse_direction(cycle(&directions, index).unwrap_or("normal")),
            parse_fill_mode(cycle(&fill_modes, index).unwrap_or("none")),
            is_paused(cycle(&play_states, index).unwrap_or("running")),
        );
        let easing = Easing::new(CalcMode::Linear, None, None).with_timing_function(
            parse_timing_function(cycle(&timing_functions, index).unwrap_or("ease"))
                .unwrap_or(TimingFunction::Linear),
        );

        for property in animated_properties(rule) {
            if let Some(animation) = build_property_animation(
                node, rule, &property, is_stop, &timing, &easing, origin, state,
            ) {
                animations.push(animation);
            }
        }
    }

    animations
}
/// The CSS properties whose `@keyframes` values this crate converts.
#[derive(Clone, Copy)]
enum CssProperty {
    Transform,
    Opacity,
    Fill,
    Stroke,
    StrokeWidth,
    StrokeDashoffset,
    StopColor,
    StopOpacity,
    D,
}

/// Builds a single property animation from one `@keyframes` rule.
fn build_property_animation(
    node: SvgNode,
    rule: &KeyframesRule,
    property: &str,
    is_stop: bool,
    timing: &Timing,
    easing: &Easing,
    origin: CssOrigin,
    state: &crate::parser::converter::State,
) -> Option<Arc<Animation>> {
    let property = property.trim();
    if property.starts_with("--") {
        log::warn!("CSS variables are not supported.");
        return None;
    }

    let Some(css_property) =
        classify_property(property, is_stop, node.tag_name() == Some(EId::Path))
    else {
        log::warn!("Unsupported CSS property in keyframes: '{}'.", property);
        return None;
    };

    let entries = property_entries(rule, property);
    if entries.iter().any(|(_, value, _)| value.contains("var(")) {
        log::warn!("CSS variables are not supported.");
        return None;
    }

    let mut easing = easing.clone();
    let kind = match css_property {
        CssProperty::Transform => {
            let keyframes = typed_keyframes(&entries, parse_transform_functions);
            if keyframes.is_empty() {
                return None;
            }
            AnimationKind::Transform(Track::new(keyframes))
        }
        CssProperty::Opacity => AnimationKind::Opacity(build_track(&entries, parse_css_opacity)?),
        CssProperty::Fill => AnimationKind::Fill(build_track(&entries, parse_css_color)?),
        CssProperty::Stroke => AnimationKind::Stroke(build_track(&entries, parse_css_color)?),
        CssProperty::StrokeWidth => AnimationKind::StrokeWidth(build_track(&entries, |value| {
            parse_css_stroke_width(value, node, state)
        })?),
        CssProperty::StrokeDashoffset => {
            AnimationKind::StrokeDashoffset(build_track(&entries, |value| {
                parse_css_stroke_dashoffset(value, node, state)
            })?)
        }
        CssProperty::StopColor => AnimationKind::StopColor(build_track(&entries, parse_css_color)?),
        CssProperty::StopOpacity => {
            AnimationKind::StopOpacity(build_track(&entries, parse_css_opacity)?)
        }
        CssProperty::D => {
            let bake = build_css_path_track(&entries)?;
            easing.calc_mode = bake.calc_mode;
            bake.kind
        }
    };

    let animation = Animation::new(
        kind,
        timing.clone(),
        easing,
        Additive::Replace,
        Accumulate::None,
        AnimationSource::Css,
        property_suppressed_by_important(node, property),
    );
    let animation = match css_property {
        CssProperty::Transform => animation.with_css_origin(origin),
        _ => animation,
    };
    Some(Arc::new(animation))
}

/// Classifies a CSS property name against the supported set.
///
/// `stop-color`/`stop-opacity` are only admitted on `<stop>` targets, while
/// `d` is only admitted on `<path>` targets.
fn classify_property(property: &str, is_stop: bool, is_path: bool) -> Option<CssProperty> {
    if property.eq_ignore_ascii_case("transform") {
        Some(CssProperty::Transform)
    } else if property.eq_ignore_ascii_case("opacity") {
        Some(CssProperty::Opacity)
    } else if property.eq_ignore_ascii_case("fill") {
        Some(CssProperty::Fill)
    } else if property.eq_ignore_ascii_case("stroke") {
        Some(CssProperty::Stroke)
    } else if property.eq_ignore_ascii_case("stroke-width") {
        Some(CssProperty::StrokeWidth)
    } else if property.eq_ignore_ascii_case("stroke-dashoffset") {
        Some(CssProperty::StrokeDashoffset)
    } else if is_stop && property.eq_ignore_ascii_case("stop-color") {
        Some(CssProperty::StopColor)
    } else if is_stop && property.eq_ignore_ascii_case("stop-opacity") {
        Some(CssProperty::StopOpacity)
    } else if is_path && property.eq_ignore_ascii_case("d") {
        Some(CssProperty::D)
    } else {
        None
    }
}

/// Collects the distinct property names animated by a rule, in first-seen order.
fn animated_properties(rule: &KeyframesRule) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for keyframe in &rule.keyframes {
        for (property, _) in &keyframe.declarations {
            let property = property.trim();
            if !names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(property))
            {
                names.push(property.to_string());
            }
        }
    }
    names
}

/// Gathers a single property's `(offset, value, timing-function)` entries,
/// sorted by offset.
fn property_entries<'r>(
    rule: &'r KeyframesRule,
    property: &str,
) -> Vec<(f32, &'r str, Option<&'r str>)> {
    let mut entries = Vec::new();
    for keyframe in &rule.keyframes {
        let Some((_, value)) = keyframe
            .declarations
            .iter()
            .find(|(name, _)| name.trim().eq_ignore_ascii_case(property))
        else {
            continue;
        };
        for &offset in &keyframe.offsets {
            entries.push((offset, value.as_str(), keyframe.timing_function.as_deref()));
        }
    }
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));
    entries
}

/// Builds a typed track, returning `None` when no keyframe value parses.
fn build_track<T: Clone>(
    entries: &[(f32, &str, Option<&str>)],
    parse: impl Fn(&str) -> Option<T>,
) -> Option<Track<T>> {
    let keyframes = typed_keyframes(entries, parse);
    (!keyframes.is_empty()).then(|| Track::new(keyframes))
}

/// Parses each entry's value into a typed keyframe, dropping unparsable ones.
fn typed_keyframes<T: Clone>(
    entries: &[(f32, &str, Option<&str>)],
    parse: impl Fn(&str) -> Option<T>,
) -> Vec<Keyframe<T>> {
    entries
        .iter()
        .copied()
        .filter_map(|(offset, value, timing)| {
            Some(Keyframe::new(
                NormalizedF32::new_clamped(offset),
                parse(value.trim())?,
                timing.and_then(parse_timing_function),
            ))
        })
        .collect()
}

/// Returns whether a winning `!important` static declaration suppresses the
/// property's animation.
fn property_suppressed_by_important(node: SvgNode, property: &str) -> bool {
    AId::from_str(property).is_some_and(|aid| {
        node.attributes()
            .iter()
            .find(|item| item.name == aid)
            .is_some_and(|item| item.important)
    })
}
