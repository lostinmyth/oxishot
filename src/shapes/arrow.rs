use crate::shapes::{Draw, Point, Style, corner_bounds, offset, seg_dist};
use std::fmt::Write;

pub struct Arrow {
    pub a: Point, // tail (drag start)
    pub b: Point, // tip (current/final point)
    pub style: Style,
}

impl Arrow {
    pub fn new(p: Point, style: Style) -> Self {
        Self { a: p, b: p, style }
    }

    /// The two outer ends of the head's lines (the head's "wings").
    /// None if the arrow is too short to have a direction.
    fn wings(&self) -> Option<(Point, Point)> {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            return None;
        }
        let (ux, uy) = (dx / len, dy / len);

        // head grows with stroke width, but never more than half the shaft
        let h = (self.style.radius * 4.0 + 10.0).min(len * 0.5);
        // ±30° from the reversed direction: cos 30° = 0.866, sin 30° = 0.5
        let (c, s) = (0.866_f32, 0.5_f32);
        let w1 = (x1 - h * (ux * c - uy * s), y1 - h * (uy * c + ux * s));
        let w2 = (x1 - h * (ux * c + uy * s), y1 - h * (uy * c - ux * s));
        Some((w1, w2))
    }
}

impl Draw for Arrow {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let _ = write!(out, "M {x0} {y0} L {x1} {y1}");
        if let Some(((wx1, wy1), (wx2, wy2))) = self.wings() {
            // one subpath: wing -> tip -> wing (gives a clean join at the tip)
            let _ = write!(out, " M {wx1} {wy1} L {x1} {y1} L {wx2} {wy2}");
        }
    }

    fn extend(&mut self, p: Point, preview: &mut String) -> bool {
        self.b = p;
        self.write_preview(preview);
        true
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        layer.capsule(self.a, self.b, self.style);
        if let Some((w1, w2)) = self.wings() {
            layer.capsule(w1, self.b, self.style);
            layer.capsule(w2, self.b, self.style);
        }
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        let r = self.style.radius + tol;
        seg_dist(p, self.a, self.b) <= r
            || self
                .wings()
                .is_some_and(|(w1, w2)| seg_dist(p, w1, self.b) <= r || seg_dist(p, w2, self.b) <= r)
    }

    fn bounds(&self) -> (Point, Point) {
        let (mut lo, mut hi) = corner_bounds(self.a, self.b, self.style.radius);
        if let Some((w1, w2)) = self.wings() {
            for w in [w1, w2] {
                let (wlo, whi) = corner_bounds(w, w, self.style.radius);
                lo = (lo.0.min(wlo.0), lo.1.min(wlo.1));
                hi = (hi.0.max(whi.0), hi.1.max(whi.1));
            }
        }
        (lo, hi)
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.a, d);
        offset(&mut self.b, d);
    }
}
