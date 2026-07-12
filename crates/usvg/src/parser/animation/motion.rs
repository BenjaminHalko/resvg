// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tiny_skia_path::Path;

use crate::NormalizedF32;
use crate::parser::svgtree::{AId, SvgNode};
use crate::tree::animation::{AnimationKind, CalcMode, Easing, MotionRotate, MotionTrack};

/// Parses an `<animateMotion>` element into a motion animation.
///
/// Only the `path` attribute form is supported. The `<mpath>` child form and the
/// `values`/`from`/`to`/`by` forms are dropped with a warning.
pub(crate) fn parse_animate_motion(node: SvgNode) -> Option<(AnimationKind, Easing)> {
    // The value-list forms are not supported.
    if node.has_attribute(AId::Values)
        || node.has_attribute(AId::From)
        || node.has_attribute(AId::To)
        || node.has_attribute(AId::By)
    {
        log::warn!("Unsupported animateMotion form.");
        return None;
    }

    // `<mpath>` is an unknown element and is dropped during parsing, so its
    // presence surfaces here as a missing `path` attribute.
    let Some(path_data) = node.attribute::<&str>(AId::Path) else {
        log::warn!("Unsupported animateMotion form.");
        return None;
    };

    let Some(path) = parse_motion_path(path_data) else {
        log::warn!("Invalid animateMotion path.");
        return None;
    };

    let easing = parse_easing(node);

    let key_points = match parse_key_points(node) {
        Ok(points) => points,
        Err(()) => {
            log::warn!("Invalid animateMotion path.");
            return None;
        }
    };

    // When `keyPoints` is present it must have exactly as many values as `keyTimes`.
    if let Some(points) = &key_points {
        let key_times_len = easing.key_times().map_or(0, |t| t.len());
        if points.len() != key_times_len {
            log::warn!("Invalid animateMotion path.");
            return None;
        }
    }

    let track = MotionTrack::new(path, key_points, parse_rotate(node));
    Some((AnimationKind::Motion(track), easing))
}

/// Parses the `path` attribute into a `tiny_skia_path::Path`.
///
/// Returns `None` for a zero-length path, i.e. one without at least a `MoveTo`
/// and a single drawing segment.
fn parse_motion_path(data: &str) -> Option<Arc<Path>> {
    let mut builder = tiny_skia_path::PathBuilder::new();
    for segment in svgtypes::SimplifyingPathParser::from(data) {
        let Ok(segment) = segment else { break };

        match segment {
            svgtypes::SimplePathSegment::MoveTo { x, y } => {
                builder.move_to(x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::LineTo { x, y } => {
                builder.line_to(x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                builder.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                builder.cubic_to(
                    x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
                );
            }
            svgtypes::SimplePathSegment::ClosePath => {
                builder.close();
            }
        }
    }

    if builder.len() < 2 {
        return None;
    }

    builder.finish().map(Arc::new)
}

/// Parses the `rotate` attribute. Defaults to a fixed angle of zero.
fn parse_rotate(node: SvgNode) -> MotionRotate {
    match node.attribute::<&str>(AId::Rotate) {
        Some("auto") => MotionRotate::Auto,
        Some("auto-reverse") => MotionRotate::AutoReverse,
        Some(value) => MotionRotate::Angle(value.trim().parse::<f32>().unwrap_or(0.0)),
        None => MotionRotate::Angle(0.0),
    }
}

/// Parses the easing parameters, defaulting `calcMode` to `paced`.
fn parse_easing(node: SvgNode) -> Easing {
    Easing::new(
        parse_calc_mode(node),
        parse_key_times(node),
        parse_key_splines(node),
    )
}

/// Parses `calcMode`, which defaults to `paced` for motion animations.
fn parse_calc_mode(node: SvgNode) -> CalcMode {
    match node.attribute::<&str>(AId::CalcMode) {
        Some("discrete") => CalcMode::Discrete,
        Some("linear") => CalcMode::Linear,
        Some("spline") => CalcMode::Spline,
        _ => CalcMode::Paced,
    }
}

/// Parses `keyPoints` into normalized offsets.
///
/// `Ok(None)` means the attribute is absent; `Err(())` means it is present but
/// malformed or contains an out-of-range value.
fn parse_key_points(node: SvgNode) -> Result<Option<Vec<NormalizedF32>>, ()> {
    let Some(value) = node.attribute::<&str>(AId::KeyPoints) else {
        return Ok(None);
    };

    let mut points = Vec::new();
    for part in value.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let number = part.parse::<f32>().map_err(|_| ())?;
        points.push(NormalizedF32::new(number).ok_or(())?);
    }

    if points.is_empty() {
        return Err(());
    }

    Ok(Some(points))
}

/// Parses `keyTimes` into normalized offsets, ignoring malformed input.
fn parse_key_times(node: SvgNode) -> Option<Vec<NormalizedF32>> {
    let value = node.attribute::<&str>(AId::KeyTimes)?;

    let mut times = Vec::new();
    for part in value.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        times.push(NormalizedF32::new(part.parse::<f32>().ok()?)?);
    }

    (!times.is_empty()).then_some(times)
}

/// Parses `keySplines` into cubic Bézier control point quadruples.
fn parse_key_splines(node: SvgNode) -> Option<Vec<[f32; 4]>> {
    let value = node.attribute::<&str>(AId::KeySplines)?;

    let mut splines = Vec::new();
    for group in value.split(';') {
        let group = group.trim();
        if group.is_empty() {
            continue;
        }

        let mut spline = [0.0f32; 4];
        let mut count = 0;
        for token in group.split([',', ' ', '\t', '\n', '\r']) {
            if token.is_empty() {
                continue;
            }
            if count == 4 {
                return None;
            }
            spline[count] = token.parse::<f32>().ok()?;
            count += 1;
        }

        if count != 4 {
            return None;
        }
        splines.push(spline);
    }

    (!splines.is_empty()).then_some(splines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::svgtree::{Document, EId};

    fn with_motion<R>(svg: &str, f: impl FnOnce(SvgNode) -> R) -> R {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let node = doc
            .root_element()
            .first_element_child()
            .unwrap()
            .first_element_child()
            .unwrap();
        assert_eq!(node.tag_name(), Some(EId::AnimateMotion));
        f(node)
    }

    const NS: &str = "http://www.w3.org/2000/svg";

    #[test]
    fn path_form_with_key_points() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion path='M0 0 L10 10' keyPoints='0;1' keyTimes='0;1'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            let (kind, _) = parse_animate_motion(node).unwrap();
            match kind {
                AnimationKind::Motion(track) => {
                    assert_eq!(track.key_points().unwrap().len(), 2);
                    assert!(track.path().len() >= 2);
                }
                _ => panic!("expected a motion animation"),
            }
        });
    }

    #[test]
    fn mpath_child_is_unsupported() {
        let svg = format!(
            "<svg xmlns='{NS}' xmlns:xlink='http://www.w3.org/1999/xlink'><rect>\
             <animateMotion><mpath xlink:href='#p'/></animateMotion>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            assert!(parse_animate_motion(node).is_none());
        });
    }

    #[test]
    fn values_form_is_unsupported() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion values='0,0;10,10' dur='1s'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            assert!(parse_animate_motion(node).is_none());
        });
    }

    #[test]
    fn from_to_form_is_unsupported() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion from='0,0' to='10,10' dur='1s'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            assert!(parse_animate_motion(node).is_none());
        });
    }

    #[test]
    fn zero_length_path_is_invalid() {
        let svg = format!("<svg xmlns='{NS}'><rect><animateMotion path='M10 10'/></rect></svg>");
        with_motion(&svg, |node| {
            assert!(parse_animate_motion(node).is_none());
        });
    }

    #[test]
    fn key_points_count_mismatch_is_invalid() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion path='M0 0 L10 10' keyPoints='0;0.5;1' keyTimes='0;1'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            assert!(parse_animate_motion(node).is_none());
        });
    }

    #[test]
    fn rotate_auto() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion path='M0 0 L10 10' rotate='auto'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            let (kind, _) = parse_animate_motion(node).unwrap();
            match kind {
                AnimationKind::Motion(track) => match track.rotate() {
                    MotionRotate::Auto => {}
                    other => panic!("expected auto, got {other:?}"),
                },
                _ => panic!("expected a motion animation"),
            }
        });
    }

    #[test]
    fn rotate_angle() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion path='M0 0 L10 10' rotate='90'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            let (kind, _) = parse_animate_motion(node).unwrap();
            match kind {
                AnimationKind::Motion(track) => match track.rotate() {
                    MotionRotate::Angle(a) => assert_eq!(a, 90.0),
                    other => panic!("expected an angle, got {other:?}"),
                },
                _ => panic!("expected a motion animation"),
            }
        });
    }

    #[test]
    fn default_calc_mode_is_paced() {
        let svg =
            format!("<svg xmlns='{NS}'><rect><animateMotion path='M0 0 L10 10'/></rect></svg>");
        with_motion(&svg, |node| {
            let (_, easing) = parse_animate_motion(node).unwrap();
            match easing.calc_mode() {
                CalcMode::Paced => {}
                other => panic!("expected paced, got {other:?}"),
            }
        });
    }

    #[test]
    fn spline_calc_mode_parses_key_splines() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animateMotion path='M0 0 L10 10' calcMode='spline' \
             keyTimes='0;1' keySplines='0 0 1 1'/>\
             </rect></svg>"
        );
        with_motion(&svg, |node| {
            let (_, easing) = parse_animate_motion(node).unwrap();
            match easing.calc_mode() {
                CalcMode::Spline => {}
                other => panic!("expected spline, got {other:?}"),
            }
            assert_eq!(easing.key_splines().unwrap().len(), 1);
        });
    }
}
