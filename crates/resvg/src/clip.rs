// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

pub fn apply(
    clip: &usvg::ClipPath,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::Pixmap,
    #[cfg(feature = "animation")] time: Option<f32>,
) {
    let mut clip_pixmap = tiny_skia::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();
    clip_pixmap.fill(tiny_skia::Color::BLACK);

    draw_children(
        clip.root(),
        tiny_skia::BlendMode::Clear,
        transform.pre_concat(clip.transform()),
        &mut clip_pixmap.as_mut(),
        #[cfg(feature = "animation")]
        time,
    );

    if let Some(clip) = clip.clip_path() {
        apply(
            clip,
            transform,
            pixmap,
            #[cfg(feature = "animation")]
            time,
        );
    }

    let mut mask = tiny_skia::Mask::from_pixmap(clip_pixmap.as_ref(), tiny_skia::MaskType::Alpha);
    mask.invert();
    pixmap.apply_mask(&mask);
}

fn draw_children(
    parent: &usvg::Group,
    mode: tiny_skia::BlendMode,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
    #[cfg(feature = "animation")] time: Option<f32>,
) {
    for child in parent.children() {
        match child {
            usvg::Node::Path(path) => {
                #[cfg(feature = "animation")]
                let overrides = time
                    .zip(path.animation())
                    .map(|(t, anim)| crate::animation::compose::sample_overrides(anim, t));

                #[cfg(feature = "animation")]
                match &overrides {
                    Some(o) => {
                        if o.hidden.unwrap_or(!path.is_visible()) {
                            continue;
                        }
                        if let Some((_, false)) = o.path {
                            continue;
                        }
                    }
                    None => {
                        if !path.is_visible() {
                            continue;
                        }
                    }
                }

                #[cfg(not(feature = "animation"))]
                if !path.is_visible() {
                    continue;
                }

                // We could use any values here. They will not be used anyway.
                let ctx = Context {
                    max_bbox: tiny_skia::IntRect::from_xywh(0, 0, 1, 1).unwrap(),
                    #[cfg(feature = "animation")]
                    time,
                };

                crate::path::fill_path(
                    path,
                    mode,
                    &ctx,
                    transform,
                    pixmap,
                    #[cfg(feature = "animation")]
                    overrides.as_ref(),
                );
            }
            usvg::Node::Text(text) => {
                draw_children(
                    text.flattened(),
                    mode,
                    transform,
                    pixmap,
                    #[cfg(feature = "animation")]
                    time,
                );
            }
            usvg::Node::Group(group) => {
                // An animated transform replaces the static group transform.
                #[cfg(feature = "animation")]
                let group_transform = time
                    .zip(group.animation())
                    .map(|(t, anim)| crate::animation::compose::sample_overrides(anim, t))
                    .and_then(|o| o.transform)
                    .unwrap_or_else(|| group.transform());
                #[cfg(not(feature = "animation"))]
                let group_transform = group.transform();

                let transform = transform.pre_concat(group_transform);

                if let Some(clip) = group.clip_path() {
                    // If a `clipPath` child also has a `clip-path`
                    // then we should render this child on a new canvas,
                    // clip it, and only then draw it to the `clipPath`.
                    clip_group(
                        group,
                        clip,
                        transform,
                        pixmap,
                        #[cfg(feature = "animation")]
                        time,
                    );
                } else {
                    draw_children(
                        group,
                        mode,
                        transform,
                        pixmap,
                        #[cfg(feature = "animation")]
                        time,
                    );
                }
            }
            _ => {}
        }
    }
}

fn clip_group(
    children: &usvg::Group,
    clip: &usvg::ClipPath,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
    #[cfg(feature = "animation")] time: Option<f32>,
) -> Option<()> {
    let mut clip_pixmap = tiny_skia::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();

    draw_children(
        children,
        tiny_skia::BlendMode::SourceOver,
        transform,
        &mut clip_pixmap.as_mut(),
        #[cfg(feature = "animation")]
        time,
    );
    apply(
        clip,
        transform,
        &mut clip_pixmap,
        #[cfg(feature = "animation")]
        time,
    );

    let mut paint = tiny_skia::PixmapPaint::default();
    paint.blend_mode = tiny_skia::BlendMode::Xor;
    pixmap.draw_pixmap(
        0,
        0,
        clip_pixmap.as_ref(),
        &paint,
        tiny_skia::Transform::identity(),
        None,
    );

    Some(())
}
