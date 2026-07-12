// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::OptionLog;

pub struct Context {
    pub max_bbox: tiny_skia::IntRect,
    /// The animation query time in seconds, when rendering an animated frame.
    #[cfg(feature = "animation")]
    pub time: Option<f32>,
}

pub fn render_nodes(
    parent: &usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    for node in parent.children() {
        render_node(node, ctx, transform, pixmap);
    }
}

pub fn render_node(
    node: &usvg::Node,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    match node {
        usvg::Node::Group(group) => {
            render_group(group, ctx, transform, pixmap);
        }
        usvg::Node::Path(path) => {
            crate::path::render(
                path,
                tiny_skia::BlendMode::SourceOver,
                ctx,
                transform,
                pixmap,
            );
        }
        usvg::Node::Image(image) => {
            crate::image::render(image, transform, pixmap);
        }
        usvg::Node::Text(text) => {
            render_group(text.flattened(), ctx, transform, pixmap);
        }
    }
}

fn render_group(
    group: &usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    // Sample the group's animations at the query time, if any.
    #[cfg(feature = "animation")]
    let overrides = ctx
        .time
        .zip(group.animation())
        .map(|(t, anim)| crate::animation::compose::sample_overrides(anim, t));

    // A group hidden by an animated `display`/`visibility` renders nothing.
    #[cfg(feature = "animation")]
    if overrides.as_ref().and_then(|o| o.hidden) == Some(true) {
        return Some(());
    }

    // A slice image is wrapped in a clip group whose rectangle matches the
    // static geometry. While sampling, the wrapped image root re-derives its own
    // clip, so the static one is bypassed.
    #[cfg(feature = "animation")]
    if ctx.time.is_some() && group.clip_path().is_some() {
        if let [usvg::Node::Group(child)] = group.children() {
            if child
                .animation()
                .and_then(|anim| anim.image())
                .is_some_and(|carrier| carrier.aspect().slice)
            {
                render_nodes(group, ctx, transform.pre_concat(group.transform()), pixmap);
                return Some(());
            }
        }
    }

    // An image root group re-derives its viewport from the sampled geometry, and
    // a zero-size static image is a placeholder that only an animation reveals.
    #[cfg(feature = "animation")]
    if let Some(carrier) = group.animation().and_then(|anim| anim.image()) {
        match ctx.time {
            Some(_) => {
                return render_animated_image(
                    group,
                    carrier,
                    ctx,
                    transform,
                    overrides.as_ref(),
                    pixmap,
                );
            }
            None => {
                if !carrier.underlying_renderable() {
                    return Some(());
                }
            }
        }
    }

    // An animated transform replaces the static group transform.
    #[cfg(feature = "animation")]
    let group_transform = overrides
        .as_ref()
        .and_then(|o| o.transform)
        .unwrap_or_else(|| group.transform());
    #[cfg(not(feature = "animation"))]
    let group_transform = group.transform();

    let transform = transform.pre_concat(group_transform);

    // A sampled opacity below 1 forces an isolation layer even when the static
    // group would not otherwise need one.
    #[cfg(feature = "animation")]
    let sampled_opacity = overrides.as_ref().and_then(|o| o.opacity);
    #[cfg(feature = "animation")]
    let force_isolation = sampled_opacity.is_some_and(|op| op < 1.0);
    #[cfg(not(feature = "animation"))]
    let force_isolation = false;

    if !group.should_isolate() && !force_isolation {
        render_nodes(group, ctx, transform, pixmap);
        return Some(());
    }

    let bbox = group.layer_bounding_box().transform(transform)?;

    let mut ibbox = if group.filters().is_empty() {
        // Convert group bbox into an integer one, expanding each side outwards by 2px
        // to make sure that anti-aliased pixels would not be clipped.
        tiny_skia::IntRect::from_xywh(
            (bbox.x().floor() as i32).checked_sub(2)?,
            (bbox.y().floor() as i32).checked_sub(2)?,
            (bbox.width().ceil() as u32).checked_add(4)?,
            (bbox.height().ceil() as u32).checked_add(4)?,
        )?
    } else {
        // The bounding box for groups with filters is special and should not be expanded by 2px,
        // because it's already acting as a clipping region.
        let bbox = bbox.to_int_rect();
        // Make sure our filter region is not bigger than 4x the canvas size.
        // This is required mainly to prevent huge filter regions that would tank the performance.
        // It should not affect the final result in any way.
        crate::geom::fit_to_rect(bbox, ctx.max_bbox)?
    };

    // Make sure our layer is not bigger than 4x the canvas size.
    // This is required to prevent huge layers.
    if group.filters().is_empty() {
        ibbox = crate::geom::fit_to_rect(ibbox, ctx.max_bbox)?;
    }

    let shift_ts = {
        // Original shift.
        let mut dx = bbox.x();
        let mut dy = bbox.y();

        // Account for subpixel positioned layers.
        dx -= bbox.x() - ibbox.x() as f32;
        dy -= bbox.y() - ibbox.y() as f32;

        tiny_skia::Transform::from_translate(-dx, -dy)
    };

    let transform = shift_ts.pre_concat(transform);

    let mut sub_pixmap = tiny_skia::Pixmap::new(ibbox.width(), ibbox.height())
        .log_none(|| log::warn!("Failed to allocate a group layer for: {:?}.", ibbox))?;

    render_nodes(group, ctx, transform, &mut sub_pixmap.as_mut());

    if !group.filters().is_empty() {
        for filter in group.filters() {
            crate::filter::apply(
                filter,
                transform,
                &mut sub_pixmap,
                #[cfg(feature = "animation")]
                ctx.time,
            );
        }
    }

    if let Some(clip_path) = group.clip_path() {
        crate::clip::apply(
            clip_path,
            transform,
            &mut sub_pixmap,
            #[cfg(feature = "animation")]
            ctx.time,
        );
    }

    if let Some(mask) = group.mask() {
        crate::mask::apply(mask, ctx, transform, &mut sub_pixmap);
    }

    #[cfg(feature = "animation")]
    let opacity = sampled_opacity.unwrap_or_else(|| group.opacity().get());
    #[cfg(not(feature = "animation"))]
    let opacity = group.opacity().get();

    let paint = tiny_skia::PixmapPaint {
        opacity,
        blend_mode: convert_blend_mode(group.blend_mode()),
        quality: tiny_skia::FilterQuality::Nearest,
    };

    pixmap.draw_pixmap(
        ibbox.x(),
        ibbox.y(),
        sub_pixmap.as_ref(),
        &paint,
        tiny_skia::Transform::identity(),
        None,
    );

    Some(())
}

/// Samples the root `viewBox` animation at `time`, returning the animated root
/// transform when the animation is active.
#[cfg(feature = "animation")]
pub fn root_view_box_transform(tree: &usvg::Tree, time: f32) -> Option<tiny_skia::Transform> {
    let animation = tree.view_box_animation()?;
    let progress = match animation.timing() {
        usvg::Timing::Smil(smil) => crate::animation::timing::smil_progress(smil, time)?,
        usvg::Timing::Css(css) => crate::animation::timing::css_progress(css, time)?,
    };
    let kind = usvg::AnimationKind::ViewBox(animation.track().clone());
    match crate::animation::interpolate::interpolate_track(&kind, animation.easing(), progress)? {
        crate::animation::interpolate::SampledValue::ViewBox(rect) => {
            Some(animation.to_transform(rect, tree.size()))
        }
        _ => None,
    }
}

/// Renders an image root group whose geometry is animated at the query time,
/// substituting the sampled viewport transform and, for `slice`, its clip.
#[cfg(feature = "animation")]
fn render_animated_image(
    group: &usvg::Group,
    carrier: &usvg::ImageCarrierState,
    ctx: &Context,
    transform: tiny_skia::Transform,
    overrides: Option<&crate::animation::compose::SampledOverrides>,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    // Components without an active track hold their static value.
    let (x, y, width, height) = overrides
        .and_then(|o| o.image_geometry)
        .map(|geometry| (geometry.x, geometry.y, geometry.w, geometry.h))
        .unwrap_or_else(|| carrier.static_quad());

    // A non-positive width or height hides the image at this frame.
    let Some(viewport) =
        usvg::image_viewport(x, y, width, height, carrier.aspect(), carrier.intrinsic_size())
    else {
        return Some(());
    };

    let image_transform = transform.pre_concat(viewport.transform);
    match viewport.clip_rect {
        Some(clip_rect) => {
            render_image_slice(group, ctx, transform, image_transform, clip_rect, pixmap)
        }
        None => {
            render_nodes(group, ctx, image_transform, pixmap);
            Some(())
        }
    }
}

/// Renders a `preserveAspectRatio="… slice"` image into an isolated layer and
/// clips it to the sampled slice rectangle.
#[cfg(feature = "animation")]
fn render_image_slice(
    group: &usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
    image_transform: tiny_skia::Transform,
    clip_rect: usvg::NonZeroRect,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    // The layer spans the covered image and the slice rectangle, so neither is
    // dropped before the slice mask crops the overflow.
    let image_bbox = group.layer_bounding_box().transform(image_transform)?;
    let clip_bbox = clip_rect.transform(transform)?;
    let bbox = union_rect(image_bbox, clip_bbox)?;

    let mut ibbox = tiny_skia::IntRect::from_xywh(
        (bbox.x().floor() as i32).checked_sub(2)?,
        (bbox.y().floor() as i32).checked_sub(2)?,
        (bbox.width().ceil() as u32).checked_add(4)?,
        (bbox.height().ceil() as u32).checked_add(4)?,
    )?;
    ibbox = crate::geom::fit_to_rect(ibbox, ctx.max_bbox)?;

    let shift = tiny_skia::Transform::from_translate(-ibbox.x() as f32, -ibbox.y() as f32);

    let mut sub_pixmap = tiny_skia::Pixmap::new(ibbox.width(), ibbox.height())
        .log_none(|| log::warn!("Failed to allocate an image layer for: {:?}.", ibbox))?;

    render_nodes(group, ctx, shift.pre_concat(image_transform), &mut sub_pixmap.as_mut());

    let mut mask = tiny_skia::Mask::new(ibbox.width(), ibbox.height())?;
    mask.fill_path(
        &tiny_skia::PathBuilder::from_rect(clip_rect.to_rect()),
        tiny_skia::FillRule::Winding,
        true,
        shift.pre_concat(transform),
    );
    sub_pixmap.apply_mask(&mask);

    pixmap.draw_pixmap(
        ibbox.x(),
        ibbox.y(),
        sub_pixmap.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    Some(())
}

/// The smallest rectangle covering both inputs.
#[cfg(feature = "animation")]
fn union_rect(a: usvg::NonZeroRect, b: usvg::NonZeroRect) -> Option<usvg::NonZeroRect> {
    let left = a.x().min(b.x());
    let top = a.y().min(b.y());
    let right = (a.x() + a.width()).max(b.x() + b.width());
    let bottom = (a.y() + a.height()).max(b.y() + b.height());
    usvg::NonZeroRect::from_xywh(left, top, right - left, bottom - top)
}

pub fn convert_blend_mode(mode: usvg::BlendMode) -> tiny_skia::BlendMode {
    match mode {
        usvg::BlendMode::Normal => tiny_skia::BlendMode::SourceOver,
        usvg::BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
        usvg::BlendMode::Screen => tiny_skia::BlendMode::Screen,
        usvg::BlendMode::Overlay => tiny_skia::BlendMode::Overlay,
        usvg::BlendMode::Darken => tiny_skia::BlendMode::Darken,
        usvg::BlendMode::Lighten => tiny_skia::BlendMode::Lighten,
        usvg::BlendMode::ColorDodge => tiny_skia::BlendMode::ColorDodge,
        usvg::BlendMode::ColorBurn => tiny_skia::BlendMode::ColorBurn,
        usvg::BlendMode::HardLight => tiny_skia::BlendMode::HardLight,
        usvg::BlendMode::SoftLight => tiny_skia::BlendMode::SoftLight,
        usvg::BlendMode::Difference => tiny_skia::BlendMode::Difference,
        usvg::BlendMode::Exclusion => tiny_skia::BlendMode::Exclusion,
        usvg::BlendMode::Hue => tiny_skia::BlendMode::Hue,
        usvg::BlendMode::Saturation => tiny_skia::BlendMode::Saturation,
        usvg::BlendMode::Color => tiny_skia::BlendMode::Color,
        usvg::BlendMode::Luminosity => tiny_skia::BlendMode::Luminosity,
    }
}
