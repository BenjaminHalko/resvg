// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Typed animation data model.
//!
//! Supports SMIL (`animate`, `animateTransform`, `animateMotion`, `set`) and CSS
//! (`@keyframes` + `animation` properties). Animations attach per-node on
//! `Group`, `Path`, and `Image`; per-gradient on paint servers; and at `Tree`
//! level for `viewBox`.
//!
//! Geometry attribute animations are baked to path-data keyframe snapshots at
//! parse time. No evaluation methods are provided; all interpolation math lives
//! in the `resvg` crate.
//!
//! The usvg writer does not serialize animations.

mod carriers;
mod gradient;
mod kinds;
mod timing;
mod tracks;
mod view_box;

pub use carriers::{
    FillCarrierState, ImageCarrierState, NodeAnimation, PathCarrierState, StrokeCarrierState,
};
pub use gradient::{GradientAnimation, SourceStop};
pub use kinds::{
    Accumulate, Additive, Animation, AnimationKind, AnimationSource, AnimationVisibility,
};
pub use timing::{
    Begin, CalcMode, CssFillMode, CssTiming, Direction, Dur, Easing, Interval, Iterations,
    KeyOffset, PlayState, RepeatCount, Restart, SmilFill, SmilTiming, StepPosition, Timing,
    TimingFunction,
};
pub use tracks::{
    Keyframe, MotionRotate, MotionTrack, PathKeyframe, PathTrack, Track, TransformBox,
    TransformFunction, TransformKind, TransformOrigin, TransformOriginValue, TransformTrack,
};
pub use view_box::ViewBoxAnimation;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn construct_animation_kinds() {
        let mut pb = tiny_skia_path::PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(1.0, 1.0);
        let path = Arc::new(pb.finish().unwrap());

        let _ = AnimationKind::Transform(TransformTrack::Smil {
            kind: TransformKind::Translate,
            keyframes: vec![],
        });
        let _ = AnimationKind::GradientTransform(TransformTrack::Css {
            keyframes: vec![],
            origin: TransformOrigin::new(
                TransformOriginValue::Length(0.0),
                TransformOriginValue::Percent(50.0),
            ),
            box_: TransformBox::ViewBox,
        });
        let _ = AnimationKind::Motion(MotionTrack::new(path, None, MotionRotate::Auto));
        let _ = AnimationKind::Opacity(Track::new(vec![]));
        let _ = AnimationKind::Fill(Track::new(vec![]));
        let _ = AnimationKind::Stroke(Track::new(vec![]));
        let _ = AnimationKind::StrokeWidth(Track::new(vec![]));
        let _ = AnimationKind::StrokeDashoffset(Track::new(vec![]));
        let _ = AnimationKind::StrokeDasharray(Track::new(vec![]));
        let _ = AnimationKind::StrokeMiterlimit(Track::new(vec![]));
        let _ = AnimationKind::StrokeLinecap(Track::new(vec![]));
        let _ = AnimationKind::StrokeLinejoin(Track::new(vec![]));
        let _ = AnimationKind::FillRule(Track::new(vec![]));
        let _ = AnimationKind::Display(Track::new(vec![]));
        let _ = AnimationKind::Visibility(Track::new(vec![]));
        let _ = AnimationKind::Path(PathTrack::new(vec![], None));
        let _ = AnimationKind::StopColor(Track::new(vec![]));
        let _ = AnimationKind::StopOpacity(Track::new(vec![]));
        let _ = AnimationKind::StopOffset(Track::new(vec![]));
        let _ = AnimationKind::GradientGeometry(Track::new(vec![]));
        let _ = AnimationKind::ViewBox(Track::new(vec![]));
        let _ = AnimationKind::ImageX(Track::new(vec![]));
        let _ = AnimationKind::ImageY(Track::new(vec![]));
        let _ = AnimationKind::ImageWidth(Track::new(vec![]));
        let _ = AnimationKind::ImageHeight(Track::new(vec![]));
    }
}
