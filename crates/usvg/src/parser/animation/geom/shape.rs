// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia_path::Path;

use crate::IsValidLength;
use crate::parser::shapes::{circle_path, ellipse_path, line_path, rect_path};
use crate::parser::svgtree::EId;

/// The resolved static geometry of a shape.
///
/// The animated attribute is overridden per keyframe; the remaining fields
/// supply the shape's other parameters.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ShapeGeometry {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) rx: f32,
    pub(crate) ry: f32,
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    pub(crate) r: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
    pub(crate) x2: f32,
    pub(crate) y2: f32,
    #[cfg(feature = "animation")]
    pub(crate) rx_is_implicit: bool,
    #[cfg(feature = "animation")]
    pub(crate) ry_is_implicit: bool,
}

impl ShapeGeometry {
    pub(crate) fn attribute(&self, attribute: &str) -> Option<f32> {
        match attribute {
            "x" => Some(self.x),
            "y" => Some(self.y),
            "width" => Some(self.width),
            "height" => Some(self.height),
            "rx" => Some(self.rx),
            "ry" => Some(self.ry),
            "cx" => Some(self.cx),
            "cy" => Some(self.cy),
            "r" => Some(self.r),
            "x1" => Some(self.x1),
            "y1" => Some(self.y1),
            "x2" => Some(self.x2),
            "y2" => Some(self.y2),
            _ => None,
        }
    }

    /// Returns a copy with `attribute` set to `value`, or `None` when the
    /// attribute is not a shape geometry scalar.
    pub(super) fn with_attribute(mut self, attribute: &str, value: f32) -> Option<Self> {
        match attribute {
            "x" => self.x = value,
            "y" => self.y = value,
            "width" => self.width = value,
            "height" => self.height = value,
            "rx" => {
                self.rx = value;
                #[cfg(feature = "animation")]
                if self.ry_is_implicit {
                    self.ry = value;
                }
            }
            "ry" => {
                self.ry = value;
                #[cfg(feature = "animation")]
                if self.rx_is_implicit {
                    self.rx = value;
                }
            }
            "cx" => self.cx = value,
            "cy" => self.cy = value,
            "r" => self.r = value,
            "x1" => self.x1 = value,
            "y1" => self.y1 = value,
            "x2" => self.x2 = value,
            "y2" => self.y2 = value,
            _ => return None,
        }
        Some(self)
    }
}

/// Builds a shape path from resolved geometry using the shared shape builders.
pub(super) fn build_shape_path(element_tag: EId, g: &ShapeGeometry) -> Option<Path> {
    match element_tag {
        EId::Rect => rect_path(g.x, g.y, g.width, g.height, g.rx, g.ry),
        EId::Circle => circle_path(g.cx, g.cy, g.r),
        EId::Ellipse => ellipse_path(g.cx, g.cy, g.rx, g.ry),
        EId::Line => line_path(g.x1, g.y1, g.x2, g.y2),
        _ => None,
    }
}

/// Reports whether a shape produces a renderable (non-degenerate) snapshot.
pub(super) fn is_shape_renderable(element_tag: EId, g: &ShapeGeometry) -> bool {
    match element_tag {
        EId::Rect => g.width.is_valid_length() && g.height.is_valid_length(),
        EId::Circle => g.r.is_valid_length(),
        EId::Ellipse => g.rx.is_valid_length() && g.ry.is_valid_length(),
        EId::Line => true,
        _ => false,
    }
}
