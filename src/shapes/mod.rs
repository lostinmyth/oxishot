pub mod arrow;
pub mod circle;
pub mod rectangle;
pub mod stroke;
pub mod text;

use arrow::Arrow;
use circle::Circle;
use rectangle::Rectangle;
use slint::Rgba8Pixel;
use stroke::Stroke;
use text::Text;

use crate::raster::Layer;

pub type Point = (f32, f32);

#[derive(Clone, Copy)]
pub struct Style {
    pub color: Rgba8Pixel,
    pub radius: f32,
}

pub trait Draw {
    fn write_preview(&self, out: &mut String);
    fn extend(&mut self, p: Point, preview: &mut String) -> bool;
    fn rasterize(&self, layer: &mut Layer);
    /// Whether `p` lies on the drawn shape, with `tol` px of slack.
    fn hit(&self, p: Point, tol: f32) -> bool;
    /// Box around everything the shape paints, turned with the shape.
    fn frame(&self) -> Frame;
    fn translate(&mut self, d: Point);
    /// Rotates the shape by `angle` radians (clockwise on screen) about `c`.
    fn rotate(&mut self, c: Point, angle: f32);
}

/// Distance from `p` to the segment `a`-`b`.
pub fn seg_dist(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (fx, fy) = (p.0 - a.0, p.1 - a.1);
    let t = ((fx * dx + fy * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
    let (ex, ey) = (fx - t * dx, fy - t * dy);
    (ex * ex + ey * ey).sqrt()
}

pub fn offset(p: &mut Point, d: Point) {
    p.0 += d.0;
    p.1 += d.1;
}

/// `p` rotated about `c`; `sc` is `angle.sin_cos()`.
pub fn rotated(p: Point, c: Point, (s, k): (f32, f32)) -> Point {
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    (c.0 + dx * k - dy * s, c.1 + dx * s + dy * k)
}

/// Axis-aligned bounds of `pts`, grown by `pad` on every side.
pub fn points_bounds(pts: impl IntoIterator<Item = Point>, pad: f32) -> (Point, Point) {
    let (lo, hi) = pts.into_iter().fold(
        (
            (f32::INFINITY, f32::INFINITY),
            (f32::NEG_INFINITY, f32::NEG_INFINITY),
        ),
        |(lo, hi), q| {
            (
                (lo.0.min(q.0), lo.1.min(q.1)),
                (hi.0.max(q.0), hi.1.max(q.1)),
            )
        },
    );
    ((lo.0 - pad, lo.1 - pad), (hi.0 + pad, hi.1 + pad))
}

/// A box with center `center`, half-size `half`, rotated by `angle` radians
/// about its center. Selection outlines follow it, so a rotated rectangle
/// gets a rotated box instead of an ever-growing axis-aligned one.
#[derive(Clone, Copy)]
pub struct Frame {
    pub center: Point,
    pub half: Point,
    pub angle: f32,
}

impl Frame {
    /// Unrotated frame covering the (min, max) corners.
    pub fn aligned((lo, hi): (Point, Point)) -> Self {
        Self {
            center: ((lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5),
            half: ((hi.0 - lo.0) * 0.5, (hi.1 - lo.1) * 0.5),
            angle: 0.0,
        }
    }

    /// Tightest frame around `pts` (grown by `pad`) whose sides run along `angle`.
    pub fn fitted(pts: impl IntoIterator<Item = Point>, pad: f32, angle: f32) -> Self {
        // measure in a frame turned back by `angle`, then turn the result forward
        let back = (-angle).sin_cos();
        let local = pts.into_iter().map(|q| rotated(q, (0.0, 0.0), back));
        let f = Self::aligned(points_bounds(local, pad));
        Self {
            center: rotated(f.center, (0.0, 0.0), angle.sin_cos()),
            angle,
            ..f
        }
    }

    /// Corners grown by `pad` on every side, clockwise from top-left (before rotation).
    pub fn corners(&self, pad: f32) -> [Point; 4] {
        let ((cx, cy), (hx, hy)) = (self.center, (self.half.0 + pad, self.half.1 + pad));
        let sc = self.angle.sin_cos();
        [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]
            .map(|(x, y)| rotated((cx + x, cy + y), self.center, sc))
    }

    /// Whether `p` lies inside the frame grown by `pad`.
    pub fn contains(&self, p: Point, pad: f32) -> bool {
        let (x, y) = rotated(p, self.center, (-self.angle).sin_cos());
        (x - self.center.0).abs() <= self.half.0 + pad
            && (y - self.center.1).abs() <= self.half.1 + pad
    }

    /// Axis-aligned (min, max) corners around the turned frame.
    pub fn aabb(&self) -> (Point, Point) {
        points_bounds(self.corners(0.0), 0.0)
    }

    /// The frame after rotating its owner by `angle` about `c`.
    pub fn rotated_about(&self, c: Point, angle: f32) -> Self {
        Self {
            center: rotated(self.center, c, angle.sin_cos()),
            half: self.half,
            angle: self.angle + angle,
        }
    }
}

pub enum Shape {
    Stroke(Stroke),
    Rectangle(Rectangle),
    Circle(Circle),
    Arrow(Arrow),
    Text(Text),
}

impl Shape {
    pub fn inner(&self) -> &dyn Draw {
        match self {
            Shape::Stroke(s) => s,
            Shape::Rectangle(r) => r,
            Shape::Circle(c) => c,
            Shape::Arrow(a) => a,
            Shape::Text(t) => t,
        }
    }

    pub fn inner_mut(&mut self) -> &mut dyn Draw {
        match self {
            Shape::Stroke(s) => s,
            Shape::Rectangle(r) => r,
            Shape::Circle(c) => c,
            Shape::Arrow(a) => a,
            Shape::Text(t) => t,
        }
    }

    pub fn write_preview(&self, out: &mut String) {
        self.inner().write_preview(out);
    }

    pub fn extend(&mut self, p: Point, out: &mut String) -> bool {
        self.inner_mut().extend(p, out)
    }

    pub fn rasterize(&self, layer: &mut Layer) {
        self.inner().rasterize(layer);
    }

    pub fn hit(&self, p: Point, tol: f32) -> bool {
        self.inner().hit(p, tol)
    }

    pub fn frame(&self) -> Frame {
        self.inner().frame()
    }

    pub fn translate(&mut self, d: Point) {
        self.inner_mut().translate(d);
    }

    pub fn rotate(&mut self, c: Point, angle: f32) {
        self.inner_mut().rotate(c, angle);
    }
}
