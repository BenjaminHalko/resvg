// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

#[cfg(feature = "animation")]
use crate::animation::compose::{self, SampledOverrides};
#[cfg(feature = "animation")]
use crate::animation::interpolate::SampledValue;
#[cfg(feature = "animation")]
use std::sync::Arc;
#[cfg(feature = "animation")]
use tiny_skia::{Path, PathBuilder, PathSegment, Point};

pub fn render(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    #[cfg(feature = "animation")]
    let overrides = ctx
        .time
        .zip(path.animation())
        .map(|(t, anim)| compose::sample_overrides(anim, t));

    #[cfg(feature = "animation")]
    match &overrides {
        Some(o) => {
            // Composed visibility/display: an animated hide wins over the static
            // value, and an animated reveal overrides a statically hidden node.
            if o.hidden.unwrap_or(!path.is_visible()) {
                return;
            }
        }
        None => {
            if !path.is_visible() {
                return;
            }
        }
    }

    #[cfg(not(feature = "animation"))]
    if !path.is_visible() {
        return;
    }

    #[cfg(feature = "animation")]
    let overrides = overrides.as_ref();

    #[cfg(feature = "animation")]
    let transform = overrides
        .and_then(|o| o.transform)
        .map(|matrix| transform.pre_concat(matrix))
        .unwrap_or(transform);

    if path.paint_order() == usvg::PaintOrder::FillAndStroke {
        fill_path(
            path,
            blend_mode,
            ctx,
            transform,
            pixmap,
            #[cfg(feature = "animation")]
            overrides,
        );
        stroke_path(
            path,
            blend_mode,
            ctx,
            transform,
            pixmap,
            #[cfg(feature = "animation")]
            overrides,
        );
    } else {
        stroke_path(
            path,
            blend_mode,
            ctx,
            transform,
            pixmap,
            #[cfg(feature = "animation")]
            overrides,
        );
        fill_path(
            path,
            blend_mode,
            ctx,
            transform,
            pixmap,
            #[cfg(feature = "animation")]
            overrides,
        );
    }
}

pub fn fill_path(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
    #[cfg(feature = "animation")] overrides: Option<&SampledOverrides>,
) -> Option<()> {
    #[cfg(feature = "animation")]
    let animated_data = effective_data(
        path,
        #[cfg(feature = "animation")]
        overrides,
    );
    #[cfg(feature = "animation")]
    let data = animated_data.as_deref().unwrap_or_else(|| path.data());
    #[cfg(not(feature = "animation"))]
    let data = path.data();

    let fill = path.fill();

    // Resolve the effective (paint, opacity, rule) and any animated solid color.
    #[cfg(feature = "animation")]
    let (base_paint, opacity, rule, anim_color) = resolve_fill(path, fill, overrides)?;
    #[cfg(not(feature = "animation"))]
    let (base_paint, opacity, rule, anim_color) = {
        let fill = fill?;
        (
            Some(fill.paint()),
            fill.opacity(),
            fill.rule(),
            None::<svgtypes::Color>,
        )
    };

    // Horizontal and vertical lines cannot be filled. Skip.
    if data.bounds().width() == 0.0 || data.bounds().height() == 0.0 {
        return None;
    }

    let rule = match rule {
        usvg::FillRule::NonZero => tiny_skia::FillRule::Winding,
        usvg::FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
    };

    let pattern_pixmap;
    let mut paint = tiny_skia::Paint::default();
    match anim_color {
        Some(color) => set_solid_paint(&mut paint, color, opacity),
        None => match base_paint? {
            usvg::Paint::Color(c) => {
                paint.set_color_rgba8(c.red, c.green, c.blue, opacity.to_u8());
            }
            usvg::Paint::LinearGradient(lg) => {
                paint.shader = convert_linear_gradient(
                    lg,
                    opacity,
                    #[cfg(feature = "animation")]
                    ctx.time,
                )?;
            }
            usvg::Paint::RadialGradient(rg) => {
                paint.shader = convert_radial_gradient(
                    rg,
                    opacity,
                    #[cfg(feature = "animation")]
                    ctx.time,
                )?;
            }
            usvg::Paint::Pattern(pattern) => {
                let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

                pattern_pixmap = patt_pix;
                paint.shader = tiny_skia::Pattern::new(
                    pattern_pixmap.as_ref(),
                    tiny_skia::SpreadMode::Repeat,
                    tiny_skia::FilterQuality::Bicubic,
                    opacity.get(),
                    patt_ts,
                );
            }
        },
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    pixmap.fill_path(data, &paint, rule, transform, None);
    Some(())
}

fn stroke_path(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
    #[cfg(feature = "animation")] overrides: Option<&SampledOverrides>,
) -> Option<()> {
    #[cfg(feature = "animation")]
    let animated_data = effective_data(
        path,
        #[cfg(feature = "animation")]
        overrides,
    );
    #[cfg(feature = "animation")]
    let data = animated_data.as_deref().unwrap_or_else(|| path.data());
    #[cfg(not(feature = "animation"))]
    let data = path.data();

    let stroke = path.stroke();

    // Resolve the effective paint, opacity, tiny-skia stroke, and animated color.
    #[cfg(feature = "animation")]
    let (base_paint, opacity, ts_stroke, anim_color) = resolve_stroke(path, stroke, overrides)?;
    #[cfg(not(feature = "animation"))]
    let (base_paint, opacity, ts_stroke, anim_color) = {
        let stroke = stroke?;
        (
            Some(stroke.paint()),
            stroke.opacity(),
            stroke.to_tiny_skia(),
            None::<svgtypes::Color>,
        )
    };

    let pattern_pixmap;
    let mut paint = tiny_skia::Paint::default();
    match anim_color {
        Some(color) => set_solid_paint(&mut paint, color, opacity),
        None => match base_paint? {
            usvg::Paint::Color(c) => {
                paint.set_color_rgba8(c.red, c.green, c.blue, opacity.to_u8());
            }
            usvg::Paint::LinearGradient(lg) => {
                paint.shader = convert_linear_gradient(
                    lg,
                    opacity,
                    #[cfg(feature = "animation")]
                    ctx.time,
                )?;
            }
            usvg::Paint::RadialGradient(rg) => {
                paint.shader = convert_radial_gradient(
                    rg,
                    opacity,
                    #[cfg(feature = "animation")]
                    ctx.time,
                )?;
            }
            usvg::Paint::Pattern(pattern) => {
                let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

                pattern_pixmap = patt_pix;
                paint.shader = tiny_skia::Pattern::new(
                    pattern_pixmap.as_ref(),
                    tiny_skia::SpreadMode::Repeat,
                    tiny_skia::FilterQuality::Bicubic,
                    opacity.get(),
                    patt_ts,
                );
            }
        },
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    pixmap.stroke_path(data, &paint, &ts_stroke, transform, None);

    Some(())
}

/// The path segments to draw after independently baked geometry tracks combine.
#[cfg(feature = "animation")]
fn effective_data(path: &usvg::Path, overrides: Option<&SampledOverrides>) -> Option<Arc<Path>> {
    let paths = &overrides?.paths;
    match paths.as_slice() {
        [] => None,
        [(path, _)] => Some(path.clone()),
        _ => merge_geometry_paths(
            path.data(),
            &paths
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
        ),
    }
}

/// Applies each geometry snapshot's changed coordinates without replacing other tracks.
#[cfg(feature = "animation")]
fn merge_geometry_paths(base: &Path, paths: &[Arc<Path>]) -> Option<Arc<Path>> {
    let mut selected: Vec<&Arc<Path>> = Vec::with_capacity(paths.len());
    for path in paths {
        if let Some(existing) = selected
            .iter_mut()
            .find(|existing| same_geometry_components(base, existing, path))
        {
            *existing = path;
        } else {
            selected.push(path);
        }
    }
    let mut builders = selected
        .iter()
        .map(|path| path.segments())
        .collect::<Vec<_>>();
    let mut output = PathBuilder::new();

    for base_segment in base.segments() {
        let samples = builders
            .iter_mut()
            .map(Iterator::next)
            .collect::<Option<Vec<_>>>()?;
        if !merge_geometry_segment(&mut output, base_segment, &samples) {
            return None;
        }
    }

    if !builders
        .iter_mut()
        .all(|segments| segments.next().is_none())
    {
        return None;
    }
    Some(Arc::new(output.finish()?))
}

#[cfg(feature = "animation")]
fn same_geometry_components(base: &Path, first: &Path, second: &Path) -> bool {
    let base_points = base.points();
    let first_points = first.points();
    let second_points = second.points();
    base_points.len() == first_points.len()
        && base_points.len() == second_points.len()
        && base_points
            .iter()
            .zip(first_points)
            .zip(second_points)
            .all(|((base, first), second)| {
                (base.x != first.x) == (base.x != second.x)
                    && (base.y != first.y) == (base.y != second.y)
            })
}

/// Merges one verb-matched segment by retaining the final change per coordinate.
#[cfg(feature = "animation")]
fn merge_geometry_segment(
    builder: &mut PathBuilder,
    base: PathSegment,
    samples: &[PathSegment],
) -> bool {
    match base {
        PathSegment::MoveTo(point) => {
            let Some(points) = matching_points(samples, |segment| match segment {
                PathSegment::MoveTo(point) => Some(*point),
                _ => None,
            }) else {
                return false;
            };
            let point = merge_geometry_point(point, &points);
            builder.move_to(point.x, point.y);
        }
        PathSegment::LineTo(point) => {
            let Some(points) = matching_points(samples, |segment| match segment {
                PathSegment::LineTo(point) => Some(*point),
                _ => None,
            }) else {
                return false;
            };
            let point = merge_geometry_point(point, &points);
            builder.line_to(point.x, point.y);
        }
        PathSegment::QuadTo(control, point) => {
            let Some(points) = matching_points(samples, |segment| match segment {
                PathSegment::QuadTo(control, point) => Some((*control, *point)),
                _ => None,
            }) else {
                return false;
            };
            let controls = points
                .iter()
                .map(|(control, _)| *control)
                .collect::<Vec<_>>();
            let ends = points.iter().map(|(_, point)| *point).collect::<Vec<_>>();
            let control = merge_geometry_point(control, &controls);
            let point = merge_geometry_point(point, &ends);
            builder.quad_to(control.x, control.y, point.x, point.y);
        }
        PathSegment::CubicTo(control1, control2, point) => {
            let Some(points) = matching_points(samples, |segment| match segment {
                PathSegment::CubicTo(control1, control2, point) => {
                    Some((*control1, *control2, *point))
                }
                _ => None,
            }) else {
                return false;
            };
            let controls1 = points
                .iter()
                .map(|(control, _, _)| *control)
                .collect::<Vec<_>>();
            let controls2 = points
                .iter()
                .map(|(_, control, _)| *control)
                .collect::<Vec<_>>();
            let ends = points
                .iter()
                .map(|(_, _, point)| *point)
                .collect::<Vec<_>>();
            let control1 = merge_geometry_point(control1, &controls1);
            let control2 = merge_geometry_point(control2, &controls2);
            let point = merge_geometry_point(point, &ends);
            builder.cubic_to(
                control1.x, control1.y, control2.x, control2.y, point.x, point.y,
            );
        }
        PathSegment::Close => {
            if !samples
                .iter()
                .all(|segment| matches!(segment, PathSegment::Close))
            {
                return false;
            }
            builder.close();
        }
    }
    true
}

/// Collects matching payloads from a verb sequence.
#[cfg(feature = "animation")]
fn matching_points<T>(
    samples: &[PathSegment],
    extract: impl Fn(&PathSegment) -> Option<T>,
) -> Option<Vec<T>> {
    samples.iter().map(extract).collect()
}

#[cfg(feature = "animation")]
fn merge_geometry_point(base: Point, points: &[Point]) -> Point {
    points.iter().fold(base, |merged, point| {
        Point::from_xy(merged.x + point.x - base.x, merged.y + point.y - base.y)
    })
}

/// Sets a solid color on `paint`, folding the color's own alpha into `opacity`.
fn set_solid_paint(paint: &mut tiny_skia::Paint, color: svgtypes::Color, opacity: usvg::Opacity) {
    let alpha = (f32::from(color.alpha) / 255.0) * opacity.get();
    let alpha = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    paint.set_color_rgba8(color.red, color.green, color.blue, alpha);
}

/// Resolves the effective fill paint, opacity, rule, and animated solid color.
///
/// The base comes from the static fill, or from the animation carrier when the
/// static fill was `none`. Returns `None` when there is nothing to fill.
#[cfg(feature = "animation")]
fn resolve_fill<'a>(
    path: &'a usvg::Path,
    fill: Option<&'a usvg::Fill>,
    overrides: Option<&SampledOverrides>,
) -> Option<(
    Option<&'a usvg::Paint>,
    usvg::Opacity,
    usvg::FillRule,
    Option<svgtypes::Color>,
)> {
    // The carrier is an animation-time fallback for a statically disabled fill;
    // a plain render (no overrides) must ignore it and use the static value.
    let carrier = overrides
        .and_then(|_| path.animation())
        .and_then(|a| a.fill());
    let (base_paint, opacity, rule) = match fill {
        Some(fill) => (Some(fill.paint()), fill.opacity(), fill.rule()),
        None => {
            let carrier = carrier?;
            (carrier.paint(), carrier.opacity(), carrier.rule())
        }
    };

    let anim_color = overrides.and_then(|o| o.fill);
    if base_paint.is_none() && anim_color.is_none() {
        return None;
    }

    let rule = overrides.and_then(|o| o.fill_rule).unwrap_or(rule);
    Some((base_paint, opacity, rule, anim_color))
}

/// Resolves the effective stroke paint, opacity, tiny-skia stroke, and animated
/// solid color.
///
/// The base comes from the static stroke, or from the animation carrier when the
/// static stroke was `none`. Returns `None` when there is nothing to stroke.
#[cfg(feature = "animation")]
fn resolve_stroke<'a>(
    path: &'a usvg::Path,
    stroke: Option<&'a usvg::Stroke>,
    overrides: Option<&SampledOverrides>,
) -> Option<(
    Option<&'a usvg::Paint>,
    usvg::Opacity,
    tiny_skia::Stroke,
    Option<svgtypes::Color>,
)> {
    // The carrier is an animation-time fallback for a statically disabled stroke;
    // a plain render (no overrides) must ignore it and use the static value.
    let carrier = overrides
        .and_then(|_| path.animation())
        .and_then(|a| a.stroke());
    let (base_paint, opacity, width, linecap, linejoin, miterlimit, dasharray, dashoffset) =
        match stroke {
            Some(stroke) => (
                Some(stroke.paint()),
                stroke.opacity(),
                stroke.width().get(),
                stroke.linecap(),
                stroke.linejoin(),
                stroke.miterlimit().get(),
                stroke.dasharray().map(<[f32]>::to_vec),
                stroke.dashoffset(),
            ),
            None => {
                let carrier = carrier?;
                (
                    carrier.paint(),
                    carrier.opacity(),
                    carrier.width(),
                    carrier.linecap(),
                    carrier.linejoin(),
                    carrier.miterlimit().get(),
                    carrier.dasharray().map(<[f32]>::to_vec),
                    carrier.dashoffset(),
                )
            }
        };

    let anim_color = overrides.and_then(|o| o.stroke);
    if base_paint.is_none() && anim_color.is_none() {
        return None;
    }

    let width = overrides.and_then(|o| o.stroke_width).unwrap_or(width);
    let miterlimit = overrides.and_then(|o| o.miterlimit).unwrap_or(miterlimit);
    let linecap = overrides.and_then(|o| o.linecap).unwrap_or(linecap);
    let linejoin = overrides.and_then(|o| o.linejoin).unwrap_or(linejoin);
    let dashoffset = overrides.and_then(|o| o.dashoffset).unwrap_or(dashoffset);
    let dasharray = overrides.and_then(|o| o.dasharray.clone()).or(dasharray);

    let ts_stroke = build_stroke(width, miterlimit, linecap, linejoin, dasharray, dashoffset);
    Some((base_paint, opacity, ts_stroke, anim_color))
}

/// Builds a tiny-skia stroke from the resolved stroke parameters.
#[cfg(feature = "animation")]
fn build_stroke(
    width: f32,
    miterlimit: f32,
    linecap: usvg::LineCap,
    linejoin: usvg::LineJoin,
    dasharray: Option<Vec<f32>>,
    dashoffset: f32,
) -> tiny_skia::Stroke {
    tiny_skia::Stroke {
        width,
        miter_limit: miterlimit,
        line_cap: match linecap {
            usvg::LineCap::Butt => tiny_skia::LineCap::Butt,
            usvg::LineCap::Round => tiny_skia::LineCap::Round,
            usvg::LineCap::Square => tiny_skia::LineCap::Square,
        },
        line_join: match linejoin {
            usvg::LineJoin::Miter => tiny_skia::LineJoin::Miter,
            usvg::LineJoin::MiterClip => tiny_skia::LineJoin::MiterClip,
            usvg::LineJoin::Round => tiny_skia::LineJoin::Round,
            usvg::LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        },
        dash: dasharray.and_then(|list| tiny_skia::StrokeDash::new(list, dashoffset)),
    }
}

fn convert_linear_gradient(
    gradient: &usvg::LinearGradient,
    opacity: usvg::Opacity,
    #[cfg(feature = "animation")] time: Option<f32>,
) -> Option<tiny_skia::Shader<'_>> {
    let (mode, points) = convert_base_gradient(
        gradient,
        opacity,
        #[cfg(feature = "animation")]
        time.zip(gradient.animation()),
    )?;

    let shader = tiny_skia::LinearGradient::new(
        (gradient.x1(), gradient.y1()).into(),
        (gradient.x2(), gradient.y2()).into(),
        points,
        mode,
        #[cfg(feature = "animation")]
        gradient_transform(gradient, time),
        #[cfg(not(feature = "animation"))]
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_radial_gradient(
    gradient: &usvg::RadialGradient,
    opacity: usvg::Opacity,
    #[cfg(feature = "animation")] time: Option<f32>,
) -> Option<tiny_skia::Shader<'_>> {
    #[cfg(feature = "animation")]
    if let Some(animation) = gradient.animation() {
        return convert_animated_radial(gradient, opacity, animation, time);
    }

    let (mode, points) = convert_base_gradient(
        gradient,
        opacity,
        #[cfg(feature = "animation")]
        None,
    )?;

    let shader = tiny_skia::RadialGradient::new(
        (gradient.fx(), gradient.fy()).into(),
        gradient.fr().get(),
        (gradient.cx(), gradient.cy()).into(),
        gradient.r().get(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

/// Rebuilds a radial gradient at the query time.
///
/// The base radius comes from `underlying_r` when a carrier was synthesized for a
/// non-positive static `r`, otherwise from the gradient's own radius. An effective
/// radius `<= 0` paints the last stop as a solid fill, per SVG's `r = 0` rule.
#[cfg(feature = "animation")]
fn convert_animated_radial<'a>(
    gradient: &'a usvg::RadialGradient,
    opacity: usvg::Opacity,
    animation: &'a usvg::GradientAnimation,
    time: Option<f32>,
) -> Option<tiny_skia::Shader<'a>> {
    let overrides = time.map(|t| sample_animation_list(animation.animations(), t));
    let geometry = overrides
        .as_ref()
        .map(|overrides| radial_geometry(gradient, animation, overrides))
        .unwrap_or_else(|| RadialGeometry {
            cx: gradient.cx(),
            cy: gradient.cy(),
            r: animation
                .underlying_r()
                .unwrap_or_else(|| gradient.r().get()),
            fx: gradient.fx(),
            fy: gradient.fy(),
            fr: gradient.fr().get(),
        });

    let sample = time.map(|t| (t, animation));
    if geometry.r <= 0.0 {
        return Some(tiny_skia::Shader::SolidColor(last_stop_color(
            gradient, opacity, sample,
        )));
    }

    let transform = overrides
        .as_ref()
        .and_then(|overrides| {
            overrides
                .transform
                .map(|transform| gradient.transform().pre_concat(transform))
        })
        .unwrap_or_else(|| gradient.transform());
    let (mode, points) = convert_base_gradient(gradient, opacity, sample)?;

    tiny_skia::RadialGradient::new(
        (geometry.fx, geometry.fy).into(),
        geometry.fr,
        (geometry.cx, geometry.cy).into(),
        geometry.r,
        points,
        mode,
        transform,
    )
}

fn convert_base_gradient(
    gradient: &usvg::BaseGradient,
    opacity: usvg::Opacity,
    #[cfg(feature = "animation")] sample: Option<(f32, &usvg::GradientAnimation)>,
) -> Option<(tiny_skia::SpreadMode, Vec<tiny_skia::GradientStop>)> {
    let mode = match gradient.spread_method() {
        usvg::SpreadMethod::Pad => tiny_skia::SpreadMode::Pad,
        usvg::SpreadMethod::Reflect => tiny_skia::SpreadMode::Reflect,
        usvg::SpreadMethod::Repeat => tiny_skia::SpreadMode::Repeat,
    };

    let mut points = Vec::with_capacity(gradient.stops().len());

    #[cfg(not(feature = "animation"))]
    for stop in gradient.stops() {
        let alpha = stop.opacity() * opacity;
        let color = tiny_skia::Color::from_rgba8(
            stop.color().red,
            stop.color().green,
            stop.color().blue,
            alpha.to_u8(),
        );
        points.push(tiny_skia::GradientStop::new(stop.offset().get(), color));
    }

    #[cfg(feature = "animation")]
    {
        let mut previous_offset = 0.0;
        for (index, stop) in gradient.stops().iter().enumerate() {
            let (color, offset) = effective_stop(stop, opacity, sample, index, previous_offset);
            previous_offset = offset;
            points.push(tiny_skia::GradientStop::new(offset, color));
        }
    }

    Some((mode, points))
}

/// Samples a gradient's animation list at `t` through the node sandwich, whose
/// `gradient_overrides` slot collects the stop and geometry tracks.
#[cfg(feature = "animation")]
fn sample_animation_list(
    animations: &[std::sync::Arc<usvg::Animation>],
    t: f32,
) -> SampledOverrides {
    let node = usvg::NodeAnimation::new(animations.to_vec(), false, None, None, None, None);
    compose::sample_overrides(&node, t)
}

#[cfg(feature = "animation")]
struct RadialGeometry {
    cx: f32,
    cy: f32,
    r: f32,
    fx: f32,
    fy: f32,
    fr: f32,
}

#[cfg(feature = "animation")]
#[derive(Clone, Copy)]
enum RadialGeometryComponent {
    Cx,
    Cy,
    R,
    Fx,
    Fy,
    Fr,
}

#[cfg(feature = "animation")]
fn radial_geometry(
    gradient: &usvg::RadialGradient,
    animation: &usvg::GradientAnimation,
    overrides: &SampledOverrides,
) -> RadialGeometry {
    let mut geometry = RadialGeometry {
        cx: gradient.cx(),
        cy: gradient.cy(),
        r: animation
            .underlying_r()
            .unwrap_or_else(|| gradient.r().get()),
        fx: gradient.fx(),
        fy: gradient.fy(),
        fr: gradient.fr().get(),
    };
    let focal_x_follows_center = gradient.fx() == gradient.cx();
    let focal_y_follows_center = gradient.fy() == gradient.cy();
    for (index, value) in &overrides.gradient_overrides {
        let Some(usvg::AnimationKind::GradientGeometry(track)) = animation
            .animations()
            .get(*index)
            .map(|animation| animation.kind())
        else {
            continue;
        };
        let SampledValue::GradientGeometry(value) = value else {
            continue;
        };
        let Some(initial) = track.keyframes().first().map(|keyframe| *keyframe.value()) else {
            continue;
        };
        let component = [
            (RadialGeometryComponent::Cx, geometry.cx),
            (RadialGeometryComponent::Cy, geometry.cy),
            (RadialGeometryComponent::R, geometry.r),
            (RadialGeometryComponent::Fx, geometry.fx),
            (RadialGeometryComponent::Fy, geometry.fy),
            (RadialGeometryComponent::Fr, geometry.fr),
        ]
        .into_iter()
        .min_by(|(_, left), (_, right)| (initial - left).abs().total_cmp(&(initial - right).abs()))
        .map(|(component, _)| component);
        match component {
            Some(RadialGeometryComponent::Cx) => {
                geometry.cx = *value;
                if focal_x_follows_center {
                    geometry.fx = *value;
                }
            }
            Some(RadialGeometryComponent::Cy) => {
                geometry.cy = *value;
                if focal_y_follows_center {
                    geometry.fy = *value;
                }
            }
            Some(RadialGeometryComponent::R) => geometry.r = *value,
            Some(RadialGeometryComponent::Fx) => geometry.fx = *value,
            Some(RadialGeometryComponent::Fy) => geometry.fy = *value,
            Some(RadialGeometryComponent::Fr) => geometry.fr = *value,
            None => {}
        }
    }
    geometry
}

/// The effective gradient transform: a sampled `gradientTransform` replaces the
/// static one, as an animated node transform replaces its base.
#[cfg(feature = "animation")]
fn gradient_transform(gradient: &usvg::BaseGradient, time: Option<f32>) -> tiny_skia::Transform {
    time.zip(gradient.animation())
        .map(|(t, animation)| sample_animation_list(animation.animations(), t))
        .and_then(|overrides| {
            overrides
                .transform
                .map(|transform| gradient.transform().pre_concat(transform))
        })
        .unwrap_or_else(|| gradient.transform())
}

/// The last stop's effective color, painted solid when a radial radius is
/// non-positive.
#[cfg(feature = "animation")]
fn last_stop_color(
    gradient: &usvg::RadialGradient,
    opacity: usvg::Opacity,
    sample: Option<(f32, &usvg::GradientAnimation)>,
) -> tiny_skia::Color {
    let index = gradient.stops().len().saturating_sub(1);
    effective_stop(&gradient.stops()[index], opacity, sample, index, 0.0).0
}

/// The tiny-skia stop for a source stop at the query time: sampled color,
/// opacity, and offset fold over the static values. The offset keeps document
/// order and is monotonically clamped to its predecessor, never re-sorted.
#[cfg(feature = "animation")]
fn effective_stop(
    stop: &usvg::Stop,
    opacity: usvg::Opacity,
    sample: Option<(f32, &usvg::GradientAnimation)>,
    index: usize,
    previous_offset: f32,
) -> (tiny_skia::Color, f32) {
    let mut color = None;
    let mut stop_opacity = None;
    let mut offset = stop.offset().get();

    if let Some((t, animation)) = sample {
        if let Some(source) = animation.source_index_of(index) {
            let overrides = sample_animation_list(animation.source_stops()[source].animations(), t);
            for (_, value) in &overrides.gradient_overrides {
                match value {
                    SampledValue::Color(sampled) => color = Some(*sampled),
                    SampledValue::Opacity(sampled) => stop_opacity = Some(*sampled),
                    SampledValue::GradientGeometry(sampled) => offset = *sampled,
                    _ => {}
                }
            }
        }
    }

    let alpha = match (color, stop_opacity) {
        (None, None) => (stop.opacity() * opacity).to_u8(),
        _ => {
            let color_alpha = color.map_or(1.0, |c| f32::from(c.alpha) / 255.0);
            let stop_alpha = stop_opacity.unwrap_or_else(|| stop.opacity().get());
            ((color_alpha * stop_alpha * opacity.get()).clamp(0.0, 1.0) * 255.0).round() as u8
        }
    };
    let (red, green, blue) = color.map_or(
        (stop.color().red, stop.color().green, stop.color().blue),
        |c| (c.red, c.green, c.blue),
    );

    (
        tiny_skia::Color::from_rgba8(red, green, blue, alpha),
        offset.clamp(previous_offset, 1.0),
    )
}

fn render_pattern_pixmap(
    pattern: &usvg::Pattern,
    ctx: &Context,
    transform: tiny_skia::Transform,
) -> Option<(tiny_skia::Pixmap, tiny_skia::Transform)> {
    let (sx, sy) = {
        let ts2 = transform.pre_concat(pattern.transform());
        ts2.get_scale()
    };

    let rect = pattern.rect();
    let img_size = tiny_skia::IntSize::from_wh(
        (rect.width() * sx).round() as u32,
        (rect.height() * sy).round() as u32,
    )?;
    let mut pixmap = tiny_skia::Pixmap::new(img_size.width(), img_size.height())?;

    let transform = tiny_skia::Transform::from_scale(sx, sy);
    crate::render::render_nodes(pattern.root(), ctx, transform, &mut pixmap.as_mut());

    let mut ts = tiny_skia::Transform::default();
    ts = ts.pre_concat(pattern.transform());
    ts = ts.pre_translate(rect.x(), rect.y());
    ts = ts.pre_scale(1.0 / sx, 1.0 / sy);

    Some((pixmap, ts))
}

#[cfg(all(test, feature = "animation"))]
mod tests {
    use std::sync::Arc;

    use tiny_skia::PathBuilder;

    use super::merge_geometry_paths;

    fn rectangle(x: f32, y: f32) -> Arc<tiny_skia::Path> {
        rectangle_with_size(x, y, 10.0, 10.0)
    }

    fn rectangle_with_size(x: f32, y: f32, width: f32, height: f32) -> Arc<tiny_skia::Path> {
        let mut builder = PathBuilder::new();
        builder.move_to(x, y);
        builder.line_to(x + width, y);
        builder.line_to(x + width, y + height);
        builder.line_to(x, y + height);
        builder.close();
        Arc::new(builder.finish().unwrap())
    }

    #[test]
    fn concurrent_geometry_tracks_combine_independent_components() {
        let base = rectangle(0.0, 0.0);
        let x = rectangle(10.0, 0.0);
        let y = rectangle(0.0, 20.0);

        let merged = merge_geometry_paths(&base, &[x, y]).unwrap();
        let expected = rectangle(10.0, 20.0);

        assert_eq!(merged.points(), expected.points());
    }

    #[test]
    fn concurrent_geometry_tracks_combine_position_and_size() {
        let base = rectangle_with_size(200.0, 135.0, 50.0, 50.0);
        let x = rectangle_with_size(25.0, 135.0, 50.0, 50.0);
        let y = rectangle_with_size(200.0, 50.0, 50.0, 50.0);
        let width = rectangle_with_size(200.0, 135.0, 400.0, 50.0);
        let height = rectangle_with_size(200.0, 135.0, 50.0, 240.0);

        let merged = merge_geometry_paths(&base, &[x, y, width, height]).unwrap();
        let expected = rectangle_with_size(25.0, 50.0, 400.0, 240.0);

        assert_eq!(merged.points(), expected.points());
    }

    #[test]
    fn later_geometry_track_replaces_the_same_component() {
        let base = rectangle(0.0, 0.0);
        let first = rectangle(10.0, 0.0);
        let second = rectangle(20.0, 0.0);

        let merged = merge_geometry_paths(&base, &[first, second]).unwrap();
        let expected = rectangle(20.0, 0.0);

        assert_eq!(merged.points(), expected.points());
    }
}
