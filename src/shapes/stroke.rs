use std::fmt::Write;

use crate::shapes::{Draw, Point, Style, corner_bounds, offset, seg_dist};

pub struct Stroke {
    pub points: Vec<(f32, f32)>,
    pub style: Style,
}

impl Stroke {
    pub fn new(p: Point, style: Style) -> Self {
        Self {
            points: vec![p],
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

    fn bounds(&self) -> (Point, Point) {
        let first = self.points[0];
        self.points.iter().fold(
            corner_bounds(first, first, self.style.radius),
            |(lo, hi), &q| {
                let (qlo, qhi) = corner_bounds(q, q, self.style.radius);
                ((lo.0.min(qlo.0), lo.1.min(qlo.1)), (hi.0.max(qhi.0), hi.1.max(qhi.1)))
            },
        )
    }

    fn translate(&mut self, d: Point) {
        for q in &mut self.points {
            offset(q, d);
        }
    }
}
