use crate::shapes::{Draw, Point, Style, corner_bounds, offset, seg_dist};
use std::fmt::Write;

pub struct Rectangle {
    pub a: Point, // corner where the drag started
    pub b: Point, // current/final corner
    pub style: Style,
}

impl Rectangle {
    pub fn new(p: Point, style: Style) -> Self {
        Self { a: p, b: p, style }
    }
}

impl Draw for Rectangle {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let _ = write!(out, "M {x0} {y0} L {x1} {y0} L {x1} {y1} L {x0} {y1} Z");
    }

    fn extend(&mut self, p: Point, preview: &mut String) -> bool {
        self.b = p;
        self.write_preview(preview);

        return true;
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        for i in 0..4 {
            layer.capsule(c[i], c[(i + 1) % 4], self.style);
        }
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let r = self.style.radius + tol;
        (0..4).any(|i| seg_dist(p, c[i], c[(i + 1) % 4]) <= r)
    }

    fn bounds(&self) -> (Point, Point) {
        corner_bounds(self.a, self.b, self.style.radius)
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.a, d);
        offset(&mut self.b, d);
    }
}
