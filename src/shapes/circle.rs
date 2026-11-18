use crate::shapes::{Draw, Point, Style};
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
}
