// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Animation timing and easing evaluation.
//!
//! The `usvg` crate parses SMIL and CSS animations into a typed data model and
//! resolves SMIL timing intervals, but performs no interpolation. This module
//! evaluates that model at a query time: [`timing`] turns a time into a
//! normalized iteration progress and [`easing`] shapes that progress.

pub(crate) mod compose;
pub(crate) mod easing;
pub(crate) mod interpolate;
pub(crate) mod timing;

#[cfg(test)]
mod cross_crate_tests {
    // Proves the `usvg` public animation API is reachable from `resvg` without
    // naming any `pub(crate)` `usvg` type: only public constructors, the public
    // `ViewBoxAnimation::to_transform` method, and the public free function
    // `usvg::image_viewport` are used.

    #[test]
    fn view_box_animation_to_transform_is_reachable() {
        let rect = usvg::NonZeroRect::from_xywh(0.0, 0.0, 10.0, 20.0).unwrap();
        let keyframe = usvg::Keyframe::new(usvg::NormalizedF32::new(0.0).unwrap(), rect, None);
        let track = usvg::Track::new(vec![keyframe]);
        let easing = usvg::Easing::new(usvg::CalcMode::Linear, None, None);
        let timing = usvg::Timing::Smil(usvg::SmilTiming::new(
            vec![usvg::Begin::Offset(0.0)],
            usvg::Dur::Seconds(1.0),
            vec![],
            None,
            None,
            usvg::SmilFill::Freeze,
            usvg::Restart::Always,
            vec![usvg::Interval::new(0.0, Some(1.0))],
        ));

        let animation =
            usvg::ViewBoxAnimation::new(track, svgtypes::AspectRatio::default(), timing, easing);

        let tree_size = usvg::Size::from_wh(100.0, 100.0).unwrap();
        let transform = animation.to_transform(rect, tree_size);

        // A 10x20 sampled viewBox scaled into a 100x100 tree scales x by 10.
        assert!(transform.sx > 0.0);
        assert!(transform.sy > 0.0);
    }

    #[test]
    fn image_viewport_is_reachable() {
        let image_size = usvg::Size::from_wh(40.0, 20.0).unwrap();
        let aspect = svgtypes::AspectRatio::default();

        let viewport = usvg::image_viewport(0.0, 0.0, 80.0, 80.0, aspect, image_size).unwrap();

        // Uniform meet-scaling of 40x20 into 80x80 doubles both axes.
        assert!((viewport.transform.sx - 2.0).abs() < 1e-4);
        assert!((viewport.transform.sy - 2.0).abs() < 1e-4);
    }
}
