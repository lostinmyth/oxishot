use crate::shapes::{Draw, Frame, Point, Scale, Style, offset, rotated, scale_box, snap_outline};
use std::fmt::Write;

/// A box (spanned by `a` and `b`, turned by `angle`) that blurs whatever is
/// beneath it: the screenshot and the shapes drawn before it.
/// `style.blur` sets the strength.
#[derive(Clone)]
pub struct Blur {
    pub a: Point, // corner where the drag started (before rotation)
    pub b: Point, // current/final corner (before rotation)
    /// Rotation about the center, in radians.
    pub angle: f32,
    pub style: Style,
}

impl Blur {
    pub fn new(p: Point, style: Style) -> Self {
        Self {
            a: p,
            b: p,
            angle: 0.0,
            style,
        }
    }

    fn center(&self) -> Point {
        ((self.a.0 + self.b.0) * 0.5, (self.a.1 + self.b.1) * 0.5)
    }

    /// The blurred region as drawn: edges on pixel boundaries when axis-aligned.
    fn region(&self) -> Frame {
        snap_outline(self.frame(), 0.0)
    }
}

impl Draw for Blur {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = self.region().corners(0.0);
        let _ = write!(out, "M {x0} {y0} L {x1} {y1} L {x2} {y2} L {x3} {y3} Z");
    }

    fn extend(&mut self, p: Point, preview: &mut String) -> bool {
        self.b = p;
        self.write_preview(preview);
        true
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        layer.blur_box(self.region(), self.style.blur);
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        self.frame().contains(p, tol)
    }

    fn frame(&self) -> Frame {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        Frame {
            center: self.center(),
            half: ((x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5),
            angle: self.angle,
        }
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.a, d);
        offset(&mut self.b, d);
    }

    fn rotate(&mut self, c: Point, angle: f32) {
        let old = self.center();
        let new = rotated(old, c, angle.sin_cos());
        self.translate((new.0 - old.0, new.1 - old.1));
        self.angle += angle;
    }

    fn scale(&mut self, s: &Scale) {
        scale_box(&mut self.a, &mut self.b, s);
    }
}
