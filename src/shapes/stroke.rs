use std::fmt::Write;

use crate::shapes::{Draw, Point, Style};

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
}
