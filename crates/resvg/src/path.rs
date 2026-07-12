// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

#[cfg(feature = "animation")]
use crate::animation::compose::{self, SampledOverrides};
#[cfg(feature = "animation")]
use crate::animation::interpolate::SampledValue;

pub fn render(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    // Sample the path's animations at the query time, if any.
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
            // A degenerate animated geometry (e.g. a grow-from-zero at t=0)
            // renders nothing.
            if let Some((_, false)) = o.path {
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
    let data = effective_data(
        path,
        #[cfg(feature = "animation")]
        overrides,
    );

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
    let data = effective_data(
        path,
        #[cfg(feature = "animation")]
        overrides,
    );

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

/// The path segments to draw: an animated geometry substitutes the static data.
fn effective_data<'a>(
    path: &'a usvg::Path,
    #[cfg(feature = "animation")] overrides: Option<&'a SampledOverrides>,
) -> &'a tiny_skia::Path {
    #[cfg(feature = "animation")]
    {
        overrides
            .and_then(|o| o.path.as_ref())
            .map_or_else(|| path.data(), |(p, _)| p.as_ref())
    }
    #[cfg(not(feature = "animation"))]
    {
        path.data()
    }
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
    let base_radius = animation
        .underlying_r()
        .unwrap_or_else(|| gradient.r().get());
    let radius = overrides
        .as_ref()
        .and_then(gradient_geometry)
        .unwrap_or(base_radius);

    let sample = time.map(|t| (t, animation));
    if radius <= 0.0 {
        return Some(tiny_skia::Shader::SolidColor(last_stop_color(
            gradient, opacity, sample,
        )));
    }

    let transform = overrides
        .as_ref()
        .and_then(|overrides| overrides.transform)
        .unwrap_or_else(|| gradient.transform());
    let (mode, points) = convert_base_gradient(gradient, opacity, sample)?;

    tiny_skia::RadialGradient::new(
        (gradient.fx(), gradient.fy()).into(),
        gradient.fr().get(),
        (gradient.cx(), gradient.cy()).into(),
        radius,
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

/// The gradient-level geometry override, applied as the radial `r`.
#[cfg(feature = "animation")]
fn gradient_geometry(overrides: &SampledOverrides) -> Option<f32> {
    overrides
        .gradient_overrides
        .iter()
        .rev()
        .find_map(|(_, value)| match value {
            SampledValue::GradientGeometry(radius) => Some(*radius),
            _ => None,
        })
}

/// The effective gradient transform: a sampled `gradientTransform` replaces the
/// static one, as an animated node transform replaces its base.
#[cfg(feature = "animation")]
fn gradient_transform(gradient: &usvg::BaseGradient, time: Option<f32>) -> tiny_skia::Transform {
    time.zip(gradient.animation())
        .map(|(t, animation)| sample_animation_list(animation.animations(), t))
        .and_then(|overrides| overrides.transform)
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
