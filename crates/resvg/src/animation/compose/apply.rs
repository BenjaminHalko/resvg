// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use svgtypes::Color;
use tiny_skia::Transform;
use usvg::{
    Accumulate, Additive, Animation, AnimationKind, AnimationVisibility, NodeAnimation,
    TimingFunction, TransformTrack,
};

use super::super::interpolate::{interpolate_track_with_timing, SampledValue};
use super::accumulate::{accumulate, add_color};
use super::sandwich::Contribution;
use super::{ImageGeometry, SampledOverrides};

/// Samples one contribution and folds it into the running overrides.
pub(super) fn fold(
    overrides: &mut SampledOverrides,
    image: &mut ImageState,
    contribution: &Contribution,
) {
    let animation = contribution.animation;
    let timing_function = animation_timing_function(animation);
    let Some(sampled) = interpolate_track_with_timing(
        animation.kind(),
        animation.easing(),
        timing_function,
        contribution.progress,
    ) else {
        return;
    };
    let sampled = match animation.accumulate() {
        Accumulate::Sum => accumulate(
            animation.kind(),
            animation.easing(),
            timing_function,
            sampled,
            contribution.iteration,
        ),
        Accumulate::None => sampled,
    };
    apply(
        overrides,
        image,
        animation.kind(),
        sampled,
        animation.additive(),
        contribution.order,
    );
}

/// Routes a sampled value into its override slot and folds by additivity.
fn apply(
    overrides: &mut SampledOverrides,
    image: &mut ImageState,
    kind: &AnimationKind,
    sampled: SampledValue,
    additive: Additive,
    order: usize,
) {
    match sampled {
        SampledValue::Transform(matrix) => {
            fold_transform(&mut overrides.transform, matrix, additive);
            if let AnimationKind::Transform(TransformTrack::Css { origin, box_, .. }) = kind {
                overrides.css_transform = Some((*origin, *box_));
            }
        }
        SampledValue::Motion(matrix) => {
            // Motion supplements the transform sandwich by post-multiplication.
            let base = overrides.transform.unwrap_or_else(Transform::identity);
            overrides.transform = Some(base.pre_concat(matrix));
        }
        SampledValue::Opacity(value) => match kind {
            AnimationKind::StopOpacity(_) => {
                push_gradient(overrides, order, SampledValue::Opacity(value));
            }
            _ => fold_scalar(&mut overrides.opacity, value, additive),
        },
        SampledValue::Color(color) => match kind {
            AnimationKind::Stroke(_) => fold_color(&mut overrides.stroke, color, additive),
            AnimationKind::StopColor(_) => {
                push_gradient(overrides, order, SampledValue::Color(color))
            }
            _ => fold_color(&mut overrides.fill, color, additive),
        },
        SampledValue::StrokeWidth(value) => {
            fold_scalar(&mut overrides.stroke_width, value, additive)
        }
        SampledValue::StrokeDashoffset(value) => {
            fold_scalar(&mut overrides.dashoffset, value, additive);
        }
        SampledValue::StrokeDasharray(values) => {
            fold_dasharray(&mut overrides.dasharray, values, additive);
        }
        SampledValue::StrokeMiterlimit(value) => {
            fold_scalar(&mut overrides.miterlimit, value, additive);
        }
        SampledValue::StrokeLinecap(cap) => overrides.linecap = Some(cap),
        SampledValue::StrokeLinejoin(join) => overrides.linejoin = Some(join),
        SampledValue::FillRule(rule) => overrides.fill_rule = Some(rule),
        SampledValue::Display(shown) => overrides.hidden = Some(!shown),
        SampledValue::Visibility(visibility) => {
            overrides.hidden = Some(!matches!(visibility, AnimationVisibility::Visible));
        }
        SampledValue::Path(path, renderable) => {
            if matches!(kind, AnimationKind::Path(track) if track.replaces_geometry()) {
                overrides.paths.clear();
            }
            overrides.path = Some((path.clone(), renderable));
            overrides.paths.push((path, renderable));
        }
        SampledValue::GradientGeometry(value) => {
            push_gradient(overrides, order, SampledValue::GradientGeometry(value));
        }
        SampledValue::ViewBox(rect) => overrides.view_box = Some(rect),
        SampledValue::ImageGeometry(value) => {
            if let Some(index) = image_component(kind) {
                image.set(index, value, additive);
            }
        }
    }
}

fn animation_timing_function(animation: &Animation) -> Option<&TimingFunction> {
    animation.easing().timing_function()
}

/// Records a gradient stop or geometry override keyed by its arrival order.
fn push_gradient(overrides: &mut SampledOverrides, index: usize, value: SampledValue) {
    overrides.gradient_overrides.push((index, value));
}

/// Maps an image-geometry kind to its quad component index (`x`, `y`, `w`, `h`).
fn image_component(kind: &AnimationKind) -> Option<usize> {
    match kind {
        AnimationKind::ImageX(_) => Some(0),
        AnimationKind::ImageY(_) => Some(1),
        AnimationKind::ImageWidth(_) => Some(2),
        AnimationKind::ImageHeight(_) => Some(3),
        _ => None,
    }
}

/// Folds a scalar: `Replace` overwrites, `Sum` adds onto the running value.
fn fold_scalar(slot: &mut Option<f32>, value: f32, additive: Additive) {
    *slot = Some(match (*slot, additive) {
        (Some(current), Additive::Sum) => current + value,
        _ => value,
    });
}

/// Folds a matrix: `Replace` overwrites, `Sum` post-multiplies (`sandwich × m`).
fn fold_transform(slot: &mut Option<Transform>, matrix: Transform, additive: Additive) {
    *slot = Some(match (*slot, additive) {
        (Some(current), Additive::Sum) => current.pre_concat(matrix),
        _ => matrix,
    });
}

/// Folds a color: `Replace` overwrites, `Sum` adds each channel with saturation.
fn fold_color(slot: &mut Option<Color>, color: Color, additive: Additive) {
    *slot = Some(match (*slot, additive) {
        (Some(current), Additive::Sum) => add_color(current, color, 1),
        _ => color,
    });
}

/// Folds a dash array: `Replace` overwrites, `Sum` adds element-wise.
fn fold_dasharray(slot: &mut Option<Vec<f32>>, values: Vec<f32>, additive: Additive) {
    *slot = Some(match (slot.take(), additive) {
        (Some(current), Additive::Sum) => {
            let len = current.len().min(values.len());
            (0..len).map(|i| current[i] + values[i]).collect()
        }
        _ => values,
    });
}

/// The running quad for `image` geometry, seeded from the static carrier so an
/// animation of one component keeps the others at their static value.
pub(super) struct ImageState {
    quad: (f32, f32, f32, f32),
    touched: bool,
    available: bool,
}

impl ImageState {
    pub(super) fn new(node_anim: &NodeAnimation) -> Self {
        match node_anim.image() {
            Some(image) => {
                let (x, y, w, h) = image.static_quad();
                Self {
                    quad: (x, y, w, h),
                    touched: false,
                    available: true,
                }
            }
            None => Self {
                quad: (0.0, 0.0, 0.0, 0.0),
                touched: false,
                available: false,
            },
        }
    }

    fn set(&mut self, index: usize, value: f32, additive: Additive) {
        let slot = match index {
            0 => &mut self.quad.0,
            1 => &mut self.quad.1,
            2 => &mut self.quad.2,
            _ => &mut self.quad.3,
        };
        *slot = match additive {
            Additive::Sum => *slot + value,
            Additive::Replace => value,
        };
        self.touched = true;
    }

    pub(super) fn finish(self) -> Option<ImageGeometry> {
        (self.available && self.touched).then_some(ImageGeometry {
            x: self.quad.0,
            y: self.quad.1,
            w: self.quad.2,
            h: self.quad.3,
        })
    }
}
