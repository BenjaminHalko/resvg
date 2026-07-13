// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parsing of SMIL `values`/`from`/`to`/`by` forms into typed keyframes.
//!
//! This is the value layer only: it produces typed [`AnimationKind`] keyframe
//! tracks together with the resolved `additive`, `accumulate` and `calcMode`.
//! Timing and interval resolution live elsewhere.

mod attributes;
mod base_value;
mod forms;
mod geometry;
mod opacity;
mod paint;
mod presentation;
mod stroke;

pub(crate) use attributes::{parse_smil_transform_values, parse_smil_values, SmilTransformType};
pub(crate) use base_value::{BaseValue, SmilValues};

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::tree::animation::{
        Accumulate, Additive, AnimationKind, AnimationVisibility, CalcMode, TransformFunction,
    };
    use crate::{FillRule, LineCap, LineJoin, Opacity, StrokeMiterlimit};

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
    fn inherited_visibility_values_stay_visible() {
        let result = parse_smil_values(
            "visibility",
            Some("inherit;hidden;inherit"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Visibility(AnimationVisibility::Visible),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Visibility(track) => {
                assert_eq!(track.keyframes().len(), 3);
                assert!(matches!(
                    track.keyframes()[0].value(),
                    AnimationVisibility::Visible
                ));
                assert!(matches!(
                    track.keyframes()[1].value(),
                    AnimationVisibility::Hidden
                ));
                assert!(matches!(
                    track.keyframes()[2].value(),
                    AnimationVisibility::Visible
                ));
            }
            other => panic!("expected visibility, got {other:?}"),
        }
    }

    #[test]
    fn inherited_display_values_stay_shown() {
        let result = parse_smil_values(
            "display",
            Some("inherit;none;inherit"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Boolean(true),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Display(track) => {
                assert_eq!(track.keyframes().len(), 3);
                assert!(*track.keyframes()[0].value());
                assert!(!*track.keyframes()[1].value());
                assert!(*track.keyframes()[2].value());
            }
            other => panic!("expected display, got {other:?}"),
        }
    }

    #[test]
    fn inherited_visibility_value_keeps_hidden_base() {
        let result = parse_smil_values(
            "visibility",
            Some("inherit"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Visibility(AnimationVisibility::Hidden),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Visibility(track) => assert!(matches!(
                track.keyframes()[0].value(),
                AnimationVisibility::Hidden
            )),
            other => panic!("expected visibility, got {other:?}"),
        }
    }

    #[test]
    fn inherited_display_value_keeps_hidden_base() {
        let result = parse_smil_values(
            "display",
            Some("inherit"),
            None,
            None,
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Boolean(false),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Display(track) => assert!(!*track.keyframes()[0].value()),
            other => panic!("expected display, got {other:?}"),
        }
    }

    #[test]
    fn discrete_display_from_to_switches_halfway() {
        let result = parse_smil_values(
            "display",
            None,
            Some("none"),
            Some("inline"),
            None,
            REPLACE,
            NONE,
            LINEAR,
            None,
            &BaseValue::Boolean(false),
        )
        .unwrap();
        match &result.kind {
            AnimationKind::Display(track) => {
                assert_eq!(track.keyframes()[1].offset().get(), 0.5);
            }
            other => panic!("expected display, got {other:?}"),
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
            SmilTransformType::Translate,
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
            AnimationKind::Transform(track) => {
                let keyframes = track.keyframes();
                assert_eq!(keyframes.len(), 2);
                assert!(matches!(
                    keyframes[1].value().as_slice(),
                    [TransformFunction::Translate(x, y)] if *x == 10.0 && *y == 20.0
                ));
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
        assert_eq!(
            BaseValue::Color(color("red")).color().unwrap(),
            color("red")
        );
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
    }
}
