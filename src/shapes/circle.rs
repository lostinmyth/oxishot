use crate::shapes::{
    Draw, Frame, Point, Scale, Style, offset, rotated, scale_box, seg_dist, snap_outline,
};
use std::fmt::Write;

/// Ellipse inscribed in the rectangle spanned by `a` and `b`, rotated by `angle`.
#[derive(Clone)]
pub struct Circle {
    pub a: Point, // corner where the drag started (before rotation)
    pub b: Point, // current/final corner (before rotation)
    /// Rotation about the center, in radians.
    pub angle: f32,
    pub style: Style,
}

impl Circle {
    pub fn new(p: Point, style: Style) -> Self {
        Self {
            a: p,
            b: p,
            angle: 0.0,
            style,
        }
    }

    /// (center, radii) of the inscribed ellipse, before rotation
    fn geometry(&self) -> (Point, Point) {
        let ((x0, y0), (x1, y1)) = (self.a, self.b);
        let c = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let r = ((x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5);
        (c, r)
    }

    /// (center, radii, angle) as drawn: pixel-snapped when axis-aligned.
    fn outline(&self) -> Frame {
        let (center, half) = self.geometry();
        let f = Frame {
            center,
            half,
            angle: self.angle,
        };
        snap_outline(f, self.style.radius)
    }
}

impl Draw for Circle {
    fn write_preview(&self, out: &mut String) {
        out.clear();
        let Frame {
            center: (cx, cy),
            half: (rx, ry),
            angle,
        } = self.outline();
        let deg = angle.to_degrees();
        let (s, k) = angle.sin_cos();
        // two half-ellipse arcs; a single full arc is degenerate in SVG
        let (lx, ly, rx_, ry_) = (cx - rx * k, cy - rx * s, cx + rx * k, cy + rx * s);
        let _ = write!(
            out,
            "M {lx} {ly} A {rx} {ry} {deg} 1 0 {rx_} {ry_} A {rx} {ry} {deg} 1 0 {lx} {ly} Z"
        );
    }

    fn extend(&mut self, p: Point, preview: &mut String) -> bool {
        self.b = p;
        self.write_preview(preview);
        true
    }

    fn rasterize(&self, layer: &mut crate::raster::Layer) {
        let f = self.outline();
        if let Some(fill) = self.style.fill {
            layer.ellipse_fill(f.center, f.half, f.angle, fill);
        }
        layer.ellipse(f.center, f.half, f.angle, self.style);
    }

    fn hit(&self, p: Point, tol: f32) -> bool {
        let (c, (rx, ry)) = self.geometry();
        let reach = self.style.radius + tol;
        // work in the ellipse's own (unrotated) frame
        let (px, py) = rotated(p, c, (-self.angle).sin_cos());
        let (cx, cy) = c;
        // flat ellipse: rasterized as a line, so hit-test it as one
        if rx < 0.5 || ry < 0.5 {
            return seg_dist((px, py), (cx - rx, cy - ry), (cx + rx, cy + ry)) <= reach;
        }
        // same first-order distance estimate as Layer::ellipse
        let (ux, uy) = ((px - cx) / rx, (py - cy) / ry);
        let k0 = (ux * ux + uy * uy).sqrt();
        let (vx, vy) = (ux / rx, uy / ry);
        if self.style.fill.is_some() && k0 <= 1.0 {
            return true;
        }
        let k1 = (vx * vx + vy * vy).sqrt();
        k1 > 1e-6 && (k0 * (k0 - 1.0) / k1).abs() <= reach
    }

    fn frame(&self) -> Frame {
        let (center, (rx, ry)) = self.geometry();
        let r = self.style.radius;
        Frame {
            center,
            half: (rx + r, ry + r),
            angle: self.angle,
        }
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.a, d);
        offset(&mut self.b, d);
    }

    fn rotate(&mut self, c: Point, angle: f32) {
        // move the center around `c`, then spin the shape about its center
        let (old, _) = self.geometry();
        let new = rotated(old, c, angle.sin_cos());
        self.translate((new.0 - old.0, new.1 - old.1));
        self.angle += angle;
    }

    fn scale(&mut self, s: &Scale) {
        scale_box(&mut self.a, &mut self.b, s);
    }
}
