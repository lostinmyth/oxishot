use crate::shapes::{Draw, Point, Style, corner_bounds, offset, seg_dist};
use std::fmt::Write;

/// Ellipse inscribed in the rectangle spanned by `a` and `b`.
pub struct Circle {
    pub a: Point, // corner where the drag started
    pub b: Point, // current/final corner
    pub style: Style,
}

impl Circle {
    pub fn new(p: Point, style: Style) -> Self {
        Self { a: p, b: p, style }
    }

    /// (center, radii) of the inscribed ellipse
    fn geometry(&self) -> (Point, Point) {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let c = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let r = ((x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5);
        (c, r)
    }
}

impl Draw for Circle {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let ((cx, cy), (rx, ry)) = self.geometry();
        // two half-ellipse arcs; a single full arc is degenerate in SVG
        let (l, r) = (cx - rx, cx + rx);
        let _ = write!(
            out,
            "M {l} {cy} A {rx} {ry} 0 1 0 {r} {cy} A {rx} {ry} 0 1 0 {l} {cy} Z"
        );
    }

    fn extend(&mut self, p: Point, preview: &mut String) -> bool {
        self.b = p;
        self.write_preview(preview);
        true
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        let (c, r) = self.geometry();
        layer.ellipse(c, r, self.style);
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        let ((cx, cy), (rx, ry)) = self.geometry();
        let reach = self.style.radius + tol;
        // flat ellipse: rasterized as a line, so hit-test it as one
        if rx < 0.5 || ry < 0.5 {
            return seg_dist(p, (cx - rx, cy - ry), (cx + rx, cy + ry)) <= reach;
        }
        // same first-order distance estimate as Layer::ellipse
        let (ux, uy) = ((p.0 - cx) / rx, (p.1 - cy) / ry);
        let k0 = (ux * ux + uy * uy).sqrt();
        let (vx, vy) = (ux / rx, uy / ry);
        let k1 = (vx * vx + vy * vy).sqrt();
        k1 > 1e-6 && (k0 * (k0 - 1.0) / k1).abs() <= reach
    }

    fn bounds(&self) -> (Point, Point) {
        corner_bounds(self.a, self.b, self.style.radius)
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.a, d);
        offset(&mut self.b, d);
    }
}
