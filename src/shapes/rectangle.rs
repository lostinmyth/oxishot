use crate::shapes::{
    Draw, Frame, Point, Scale, Style, offset, rotated, scale_box, seg_dist, snap_outline,
};
use std::fmt::Write;

#[derive(Clone)]
pub struct Rectangle {
    pub a: Point, // corner where the drag started (before rotation)
    pub b: Point, // current/final corner (before rotation)
    /// Rotation about the center of `a`-`b`, in radians.
    pub angle: f32,
    pub style: Style,
}

impl Rectangle {
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

    /// The four corners in drawing order, rotated into place.
    fn corners(&self) -> [Point; 4] {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let (c, sc) = (self.center(), self.angle.sin_cos());
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|p| rotated(p, c, sc))
    }

    /// The stroke's centerline as drawn: pixel-snapped when axis-aligned.
    fn outline(&self) -> Frame {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let f = Frame {
            center: self.center(),
            half: ((x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5),
            angle: self.angle,
        };
        snap_outline(f, self.style.radius)
    }
}

impl Draw for Rectangle {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = self.outline().corners(0.0);
        let _ = write!(out, "M {x0} {y0} L {x1} {y1} L {x2} {y2} L {x3} {y3} Z");
    }

    fn extend(&mut self, p: Point, preview: &mut String) -> bool {
        self.b = p;
        self.write_preview(preview);

        return true;
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        let f = self.outline();
        layer.box_outline(f.center, f.half, f.angle, self.style);
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        let c = self.corners();
        let r = self.style.radius + tol;
        (0..4).any(|i| seg_dist(p, c[i], c[(i + 1) % 4]) <= r)
    }

    fn frame(&self) -> Frame {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let r = self.style.radius;
        Frame {
            center: self.center(),
            half: ((x1 - x0).abs() * 0.5 + r, (y1 - y0).abs() * 0.5 + r),
            angle: self.angle,
        }
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.a, d);
        offset(&mut self.b, d);
    }

    fn rotate(&mut self, c: Point, angle: f32) {
        // move the center around `c`, then spin the shape about its center
        let old = self.center();
        let new = rotated(old, c, angle.sin_cos());
        self.translate((new.0 - old.0, new.1 - old.1));
        self.angle += angle;
    }

    fn scale(&mut self, s: &Scale) {
        scale_box(&mut self.a, &mut self.b, s);
    }
}
