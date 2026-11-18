use std::fmt::Write;

use crate::shapes::{Draw, Frame, Point, Scale, Style, offset, rotated, seg_dist};

#[derive(Clone)]
pub struct Stroke {
    pub points: Vec<(f32, f32)>,
    /// Total rotation applied so far (radians). The points already include it;
    /// it only keeps the selection box turned with the stroke.
    pub angle: f32,
    pub style: Style,
}

impl Stroke {
    pub fn new(p: Point, style: Style) -> Self {
        Self {
            points: vec![p],
            angle: 0.0,
            style,
        }
    }
}

impl Draw for Stroke {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let (x, y) = self.points[0];
        let _ = write!(out, "M {x} {y} L {x} {y}");
        for &(x, y) in &self.points[1..] {
            let _ = write!(out, " L {x} {y}");
        }
    }

    fn extend(&mut self, (x, y): Point, preview: &mut String) -> bool {
        let &(lx, ly) = self.points.last().unwrap();
        if (x - lx).powi(2) + (y - ly).powi(2) < 1.0 {
            return false;
        }
        self.points.push((x, y));
        let _ = write!(preview, " L {x} {y}");
        true
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        let pts = &self.points;

        if pts.len() == 1 {
            layer.capsule(pts[0], pts[0], self.style);
        }
        for w in pts.windows(2) {
            layer.capsule(w[0], w[1], self.style);
        }
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        let r = self.style.radius + tol;
        let pts = &self.points;
        if pts.len() == 1 {
            return seg_dist(p, pts[0], pts[0]) <= r;
        }
        pts.windows(2).any(|w| seg_dist(p, w[0], w[1]) <= r)
    }

    // the box keeps the orientation the stroke was drawn in, turned with it
    fn frame(&self) -> Frame {
        Frame::fitted(self.points.iter().copied(), self.style.radius, self.angle)
    }

    fn translate(&mut self, d: Point) {
        for q in &mut self.points {
            offset(q, d);
        }
    }

    fn rotate(&mut self, c: Point, angle: f32) {
        let sc = angle.sin_cos();
        for q in &mut self.points {
            *q = rotated(*q, c, sc);
        }
        self.angle += angle;
    }

    fn scale(&mut self, s: &Scale) {
        for q in &mut self.points {
            *q = s.apply(*q);
        }
    }
}
