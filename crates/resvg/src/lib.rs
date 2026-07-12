// Copyright 2017 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
[resvg](https://github.com/linebender/resvg) is an SVG rendering library.
*/

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::identity_op)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::wrong_self_convention)]

pub use tiny_skia;
pub use usvg;

#[cfg(feature = "animation")]
mod animation;
mod clip;
mod filter;
mod geom;
mod image;
mod mask;
mod path;
mod render;

/// Renders a tree onto the pixmap.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// The produced content is in the sRGB color space.
pub fn render(
    tree: &usvg::Tree,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    let target_size = tiny_skia::IntSize::from_wh(pixmap.width(), pixmap.height()).unwrap();
    let max_bbox = tiny_skia::IntRect::from_xywh(
        -(target_size.width() as i32) * 2,
        -(target_size.height() as i32) * 2,
        target_size.width() * 5,
        target_size.height() * 5,
    )
    .unwrap();

    let ctx = render::Context {
        max_bbox,
        #[cfg(feature = "animation")]
        time: None,
    };
    render::render_nodes(tree.root(), &ctx, transform, pixmap);
}

/// Renders a tree at a specific animation time.
///
/// `time` is in seconds and may be negative. Non-finite values (`NaN`, `±∞`)
/// are treated as `0.0`.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// Note: filter/mask/clip regions and isolated-layer bounding boxes use
/// load-time geometry; `objectBoundingBox`-derived resolutions are not
/// re-derived per frame.
///
/// An animated root `viewBox` is applied as an additional root transform; the
/// static `viewBox` transform is assumed to be the identity, so a document whose
/// static `viewBox` differs from its size is not re-derived per frame.
///
/// The produced content is in the sRGB color space.
#[cfg(feature = "animation")]
pub fn render_at(
    tree: &usvg::Tree,
    time: f32,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    let target_size = tiny_skia::IntSize::from_wh(pixmap.width(), pixmap.height()).unwrap();
    let max_bbox = tiny_skia::IntRect::from_xywh(
        -(target_size.width() as i32) * 2,
        -(target_size.height() as i32) * 2,
        target_size.width() * 5,
        target_size.height() * 5,
    )
    .unwrap();

    let ctx = render::Context {
        max_bbox,
        time: Some(time.is_finite().then_some(time).unwrap_or(0.0)),
    };

    // An active root `viewBox` animation replaces the root transform.
    let transform = match ctx.time.and_then(|t| render::root_view_box_transform(tree, t)) {
        Some(view_box_ts) => transform.pre_concat(view_box_ts),
        None => transform,
    };

    render::render_nodes(tree.root(), &ctx, transform, pixmap);
}

/// Renders a node onto the pixmap.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// The expected pixmap size can be retrieved from `usvg::Node::abs_layer_bounding_box()`.
///
/// Returns `None` when `node` has a zero size.
///
/// The produced content is in the sRGB color space.
pub fn render_node(
    node: &usvg::Node,
    mut transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let bbox = node.abs_layer_bounding_box()?;

    let target_size = tiny_skia::IntSize::from_wh(pixmap.width(), pixmap.height()).unwrap();
    let max_bbox = tiny_skia::IntRect::from_xywh(
        -(target_size.width() as i32) * 2,
        -(target_size.height() as i32) * 2,
        target_size.width() * 5,
        target_size.height() * 5,
    )
    .unwrap();

    transform = transform.pre_translate(-bbox.x(), -bbox.y());

    let ctx = render::Context {
        max_bbox,
        #[cfg(feature = "animation")]
        time: None,
    };
    render::render_node(node, &ctx, transform, pixmap);

    Some(())
}

pub(crate) trait OptionLog {
    fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
    #[inline]
    fn log_none<F: FnOnce()>(self, f: F) -> Self {
        self.or_else(|| {
            f();
            None
        })
    }
}
