// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use super::motion::{distance, ArcLength};
    use super::*;

    use tiny_skia::{PathBuilder, Point};
    use usvg::{
        CalcMode, Keyframe, MotionRotate, MotionTrack, NormalizedF32, PathTrack, Track,
        TransformFunction,
    };

    fn n(v: f32) -> NormalizedF32 {
        NormalizedF32::new_clamped(v)
    }

    fn linear() -> Easing {
        Easing::new(CalcMode::Linear, None, None)
    }

    fn paced() -> Easing {
        Easing::new(CalcMode::Paced, None, None)
    }

    fn discrete() -> Easing {
        Easing::new(CalcMode::Discrete, None, None)
    }

    fn scalar_track(values: &[(f32, f32)]) -> Track<f32> {
        Track::new(
            values
                .iter()
                .map(|&(offset, value)| Keyframe::new(n(offset), value, None))
                .collect(),
        )
    }

    fn approx(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() < tol, "expected {b}, got {a}");
    }

    fn expect_scalar(value: Option<SampledValue>) -> f32 {
        match value {
            Some(SampledValue::StrokeWidth(v)) => v,
            other => panic!("expected a stroke width, got {other:?}"),
        }
    }

    fn expect_transform(value: Option<SampledValue>) -> Transform {
        match value {
            Some(SampledValue::Transform(t)) => t,
            other => panic!("expected a transform, got {other:?}"),
        }
    }

    fn approx_transform(a: Transform, b: Transform, tol: f32) {
        approx(a.sx, b.sx, tol);
        approx(a.ky, b.ky, tol);
        approx(a.kx, b.kx, tol);
        approx(a.sy, b.sy, tol);
        approx(a.tx, b.tx, tol);
        approx(a.ty, b.ty, tol);
    }

    #[test]
    fn scalar_linear_midpoint() {
        let kind = AnimationKind::StrokeWidth(scalar_track(&[(0.0, 0.0), (1.0, 100.0)]));
        approx(
            expect_scalar(interpolate_track(&kind, &linear(), 0.25)),
            25.0,
            1e-4,
        );
    }

    #[test]
    fn single_keyframe_is_constant() {
        // A `set`-style single-keyframe track holds its one value everywhere.
        let kind = AnimationKind::StrokeWidth(scalar_track(&[(0.0, 7.0)]));
        approx(
            expect_scalar(interpolate_track(&kind, &linear(), 0.0)),
            7.0,
            1e-4,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &linear(), 0.5)),
            7.0,
            1e-4,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &linear(), 1.0)),
            7.0,
            1e-4,
        );
    }

    #[test]
    fn paced_scalar_spacing() {
        // Distances 10 and 90 across [0,10,100]; time follows distance, not the
        // uniform offsets that linear interpolation would honor.
        let kind =
            AnimationKind::StrokeWidth(scalar_track(&[(0.0, 0.0), (0.5, 10.0), (1.0, 100.0)]));
        let easing = paced();

        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.05)),
            5.0,
            1e-3,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.55)),
            55.0,
            1e-3,
        );

        // The linear reading at the same progress differs, proving paced spacing.
        approx(
            expect_scalar(interpolate_track(&kind, &linear(), 0.05)),
            1.0,
            1e-3,
        );
    }

    #[test]
    fn spline_easing_between_keyframes() {
        // An ease-in spline slows the segment start, so the value trails linear.
        let spline = [0.42, 0.0, 1.0, 1.0];
        let easing = Easing::new(CalcMode::Spline, None, Some(vec![spline]));
        let kind = AnimationKind::StrokeWidth(scalar_track(&[(0.0, 0.0), (1.0, 100.0)]));

        let value = expect_scalar(interpolate_track(&kind, &easing, 0.3));
        let expected = 100.0 * super::super::easing::cubic_bezier(0.42, 0.0, 1.0, 1.0, 0.3);
        approx(value, expected, 1e-3);
        assert!(value < 30.0, "ease-in should trail linear, got {value}");
    }

    #[test]
    fn discrete_stepping_holds_last() {
        // Values step on half-open intervals and the last holds past its offset.
        let kind =
            AnimationKind::StrokeWidth(scalar_track(&[(0.0, 10.0), (0.4, 20.0), (0.8, 30.0)]));
        let easing = discrete();

        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.0)),
            10.0,
            1e-4,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.39)),
            10.0,
            1e-4,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.4)),
            20.0,
            1e-4,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.79)),
            20.0,
            1e-4,
        );
        approx(
            expect_scalar(interpolate_track(&kind, &easing, 0.8)),
            30.0,
            1e-4,
        );
        // The last value holds from 0.8 to the end of the simple duration.
        approx(
            expect_scalar(interpolate_track(&kind, &easing, 1.0)),
            30.0,
            1e-4,
        );
    }

    #[test]
    fn visibility_holds_source_value_at_discrete_boundary() {
        let kind = AnimationKind::Visibility(Track::new(vec![
            Keyframe::new(n(0.0), usvg::AnimationVisibility::Visible, None),
            Keyframe::new(n(0.5), usvg::AnimationVisibility::Hidden, None),
        ]));
        let easing = discrete();

        assert!(matches!(
            interpolate_track(&kind, &easing, 0.5),
            Some(SampledValue::Visibility(usvg::AnimationVisibility::Visible))
        ));
        assert!(matches!(
            interpolate_track(&kind, &easing, 0.5001),
            Some(SampledValue::Visibility(usvg::AnimationVisibility::Hidden))
        ));
    }

    #[test]
    fn smil_rotate_past_180_uses_param_lerp() {
        // 0 -> 270 lerps the angle to 135 at the midpoint, not the -45 a
        // shortest-arc matrix blend would take.
        let kind = AnimationKind::Transform(Track::new(vec![
            Keyframe::new(n(0.0), vec![TransformFunction::Rotate(0.0)], None),
            Keyframe::new(n(1.0), vec![TransformFunction::Rotate(270.0)], None),
        ]));

        let sampled = expect_transform(interpolate_track(&kind, &linear(), 0.5));
        approx_transform(sampled, Transform::from_rotate_at(135.0, 0.0, 0.0), 1e-4);
        // The shortest-arc blend (-45) would give the opposite-sign shear.
        assert!(
            sampled.ky > 0.0,
            "135deg keeps a positive sin, got {}",
            sampled.ky
        );
    }

    #[test]
    fn paced_rotate_spacing_by_angle() {
        // Angle deltas 90 and 270 across [0,90,360] with a constant center.
        let kind = AnimationKind::Transform(Track::new(vec![
            Keyframe::new(n(0.0), vec![TransformFunction::Rotate(0.0)], None),
            Keyframe::new(n(0.5), vec![TransformFunction::Rotate(90.0)], None),
            Keyframe::new(n(1.0), vec![TransformFunction::Rotate(360.0)], None),
        ]));

        // At quarter distance the angle is exactly 90 (end of the first delta).
        let at_quarter = expect_transform(interpolate_track(&kind, &paced(), 0.25));
        approx_transform(at_quarter, Transform::from_rotate_at(90.0, 0.0, 0.0), 1e-3);
        // Linear at the same progress would only reach 45 degrees.
        let linear_quarter = expect_transform(interpolate_track(&kind, &linear(), 0.25));
        approx_transform(
            linear_quarter,
            Transform::from_rotate_at(45.0, 0.0, 0.0),
            1e-3,
        );
    }

    #[test]
    fn paced_rotate_varying_center_falls_back_to_linear() {
        let kind = AnimationKind::Transform(Track::new(vec![
            Keyframe::new(
                n(0.0),
                vec![
                    TransformFunction::Translate(0.0, 0.0),
                    TransformFunction::Rotate(0.0),
                    TransformFunction::Translate(0.0, 0.0),
                ],
                None,
            ),
            Keyframe::new(
                n(1.0),
                vec![
                    TransformFunction::Translate(10.0, 10.0),
                    TransformFunction::Rotate(90.0),
                    TransformFunction::Translate(-10.0, -10.0),
                ],
                None,
            ),
        ]));

        // A varying center has no principled paced metric: fall back to linear.
        let sampled = expect_transform(interpolate_track(&kind, &paced(), 0.5));
        approx_transform(sampled, Transform::from_rotate_at(45.0, 5.0, 5.0), 1e-3);
    }

    #[test]
    fn css_compatible_transform_lerps() {
        let keyframes = vec![
            Keyframe::new(n(0.0), vec![TransformFunction::Translate(0.0, 0.0)], None),
            Keyframe::new(n(1.0), vec![TransformFunction::Translate(100.0, 0.0)], None),
        ];
        let kind = AnimationKind::Transform(Track::new(keyframes));

        let sampled = expect_transform(interpolate_track(&kind, &linear(), 0.5));
        approx_transform(sampled, Transform::from_translate(50.0, 0.0), 1e-4);
    }

    #[test]
    fn css_incompatible_transform_steps_and_warns() {
        let keyframes = vec![
            Keyframe::new(n(0.0), vec![TransformFunction::Translate(0.0, 0.0)], None),
            Keyframe::new(n(1.0), vec![TransformFunction::Scale(2.0, 2.0)], None),
        ];
        let kind = AnimationKind::Transform(Track::new(keyframes));

        // Structurally incompatible lists step to the low keyframe (translate 0).
        let sampled = expect_transform(interpolate_track(&kind, &linear(), 0.5));
        approx_transform(sampled, Transform::from_translate(0.0, 0.0), 1e-4);
    }

    #[test]
    fn color_channels_lerp() {
        let track = Track::new(vec![
            Keyframe::new(n(0.0), Color::new_rgba(255, 0, 0, 255), None),
            Keyframe::new(n(1.0), Color::new_rgba(0, 0, 255, 255), None),
        ]);
        let kind = AnimationKind::Fill(track);
        match interpolate_track(&kind, &linear(), 0.5) {
            Some(SampledValue::Color(c)) => {
                assert_eq!(c.red, 128);
                assert_eq!(c.green, 0);
                assert_eq!(c.blue, 128);
                assert_eq!(c.alpha, 255);
            }
            other => panic!("expected a color, got {other:?}"),
        }
    }

    #[test]
    fn dasharray_lerps_element_wise() {
        let track = Track::new(vec![
            Keyframe::new(n(0.0), vec![4.0, 4.0], None),
            Keyframe::new(n(1.0), vec![8.0, 12.0], None),
        ]);
        let kind = AnimationKind::StrokeDasharray(track);
        match interpolate_track(&kind, &linear(), 0.5) {
            Some(SampledValue::StrokeDasharray(dashes)) => {
                approx(dashes[0], 6.0, 1e-4);
                approx(dashes[1], 8.0, 1e-4);
            }
            other => panic!("expected a dasharray, got {other:?}"),
        }
    }

    #[test]
    fn viewbox_components_lerp() {
        let a = NonZeroRect::from_xywh(0.0, 0.0, 10.0, 10.0).unwrap();
        let b = NonZeroRect::from_xywh(0.0, 0.0, 20.0, 30.0).unwrap();
        let track = Track::new(vec![
            Keyframe::new(n(0.0), a, None),
            Keyframe::new(n(1.0), b, None),
        ]);
        let kind = AnimationKind::ViewBox(track);
        match interpolate_track(&kind, &linear(), 0.5) {
            Some(SampledValue::ViewBox(rect)) => {
                approx(rect.width(), 15.0, 1e-4);
                approx(rect.height(), 20.0, 1e-4);
            }
            other => panic!("expected a view box, got {other:?}"),
        }
    }

    #[test]
    fn discrete_enum_steps() {
        let track = Track::new(vec![
            Keyframe::new(n(0.0), LineCap::Butt, None),
            Keyframe::new(n(0.5), LineCap::Round, None),
        ]);
        let kind = AnimationKind::StrokeLinecap(track);
        match interpolate_track(&kind, &discrete(), 0.6) {
            Some(SampledValue::StrokeLinecap(cap)) => assert_eq!(cap, LineCap::Round),
            other => panic!("expected a line cap, got {other:?}"),
        }
    }

    fn rect_path(x: f32, y: f32, width: f32, height: f32) -> Arc<Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(x, y);
        builder.line_to(x + width, y);
        builder.line_to(x + width, y + height);
        builder.line_to(x, y + height);
        builder.close();
        Arc::new(builder.finish().unwrap())
    }

    #[test]
    fn path_grow_from_zero() {
        // A degenerate (zero-width) first keyframe with matching verbs grows into
        // a real rectangle: renderable at every t > 0, invisible only at t = 0.
        let degenerate = PathKeyframe::new(n(0.0), rect_path(10.0, 10.0, 0.0, 20.0), false, None);
        let real = PathKeyframe::new(n(1.0), rect_path(10.0, 10.0, 40.0, 20.0), true, None);
        let kind = AnimationKind::Path(PathTrack::new(vec![degenerate, real], None));

        let mid = interpolate_track(&kind, &linear(), 0.5);
        let (mid_path, mid_renderable) = match mid {
            Some(SampledValue::Path(path, renderable)) => (path, renderable),
            other => panic!("expected a path, got {other:?}"),
        };
        assert!(mid_renderable, "midpoint of a grow must render");

        // The point-wise midpoint equals the geometry built at the mid width (20).
        let expected = rect_path(10.0, 10.0, 20.0, 20.0);
        let sampled_points = mid_path.points();
        let expected_points = expected.points();
        assert_eq!(sampled_points.len(), expected_points.len());
        for (got, want) in sampled_points.iter().zip(expected_points.iter()) {
            approx(got.x, want.x, 1e-4);
            approx(got.y, want.y, 1e-4);
        }

        // Exactly at t = 0 the frame rests on the degenerate keyframe.
        match interpolate_track(&kind, &linear(), 0.0) {
            Some(SampledValue::Path(_, renderable)) => {
                assert!(!renderable, "the degenerate start must not render");
            }
            other => panic!("expected a path, got {other:?}"),
        }
    }

    // Needs the PathKeyframe constructor.
    use usvg::PathKeyframe;

    fn line_path(points: &[(f32, f32)]) -> Arc<Path> {
        let mut builder = PathBuilder::new();
        let (x0, y0) = points[0];
        builder.move_to(x0, y0);
        for &(x, y) in &points[1..] {
            builder.line_to(x, y);
        }
        Arc::new(builder.finish().unwrap())
    }

    fn cubic_path() -> Arc<Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.cubic_to(0.0, 100.0, 100.0, 100.0, 100.0, 0.0);
        Arc::new(builder.finish().unwrap())
    }

    /// A dense-sampling reference length of the fixed cubic above.
    fn cubic_reference_length() -> f32 {
        let steps = 20_000;
        let mut previous = Point::from_xy(0.0, 0.0);
        let mut total = 0.0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            // Bezier (0,0) (0,100) (100,100) (100,0).
            let x = 3.0 * mt * t * t * 100.0 + t * t * t * 100.0;
            let y = 3.0 * mt * mt * t * 100.0 + 3.0 * mt * t * t * 100.0;
            let point = Point::from_xy(x, y);
            total += distance(previous, point);
            previous = point;
        }
        total
    }

    #[test]
    fn motion_straight_baseline_length() {
        // Two axis-aligned segments: 100 across then 100 down = 200 exactly.
        let table =
            ArcLength::build(&line_path(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)])).unwrap();
        approx(table.total, 200.0, 1e-3);
    }

    #[test]
    fn motion_quadratic_length_matches_reference() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.quad_to(50.0, 100.0, 100.0, 0.0);
        let path = builder.finish().unwrap();
        let table = ArcLength::build(&path).unwrap();

        let steps = 20_000;
        let mut previous = Point::from_xy(0.0, 0.0);
        let mut reference = 0.0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            let x = 2.0 * mt * t * 50.0 + t * t * 100.0;
            let y = 2.0 * mt * t * 100.0;
            let point = Point::from_xy(x, y);
            reference += distance(previous, point);
            previous = point;
        }
        assert!(
            (table.total - reference).abs() / reference < 0.005,
            "quadratic length {} vs reference {reference}",
            table.total
        );
    }

    #[test]
    fn motion_cubic_length_within_tolerance() {
        let table = ArcLength::build(&cubic_path()).unwrap();
        let reference = cubic_reference_length();
        assert!(
            (table.total - reference).abs() / reference < 0.005,
            "cubic length {} vs reference {reference}",
            table.total
        );
    }

    #[test]
    fn motion_midpoint_is_arc_length_centered() {
        // The fixed-angle motion transform carries the sampled point in tx/ty.
        let track = MotionTrack::new(cubic_path(), None, MotionRotate::Angle(0.0));
        let kind = AnimationKind::Motion(track);
        let sampled = match interpolate_track(&kind, &paced(), 0.5) {
            Some(SampledValue::Motion(t)) => t,
            other => panic!("expected a motion transform, got {other:?}"),
        };

        // The half-length point of this symmetric curve sits at its apex x = 50.
        let table = ArcLength::build(&cubic_path()).unwrap();
        let (expected, _) = table.sample(table.total * 0.5);
        approx(sampled.tx, expected.x, 1e-2);
        approx(sampled.ty, expected.y, 1e-2);
        approx(sampled.tx, 50.0, 1e-2);
    }

    #[test]
    fn motion_tangent_drives_auto_rotation() {
        // A 45-degree diagonal yields a 45-degree auto rotation at every point.
        let track = MotionTrack::new(
            line_path(&[(0.0, 0.0), (100.0, 100.0)]),
            None,
            MotionRotate::Auto,
        );
        let kind = AnimationKind::Motion(track);
        let sampled = match interpolate_track(&kind, &paced(), 0.5) {
            Some(SampledValue::Motion(t)) => t,
            other => panic!("expected a motion transform, got {other:?}"),
        };
        // from_rotate(45) has sx = cos(45) = 0.7071.
        approx(sampled.sx, 45.0_f32.to_radians().cos(), 1e-3);
        approx(sampled.tx, 50.0, 1e-3);
        approx(sampled.ty, 50.0, 1e-3);
    }
}
