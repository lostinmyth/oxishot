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
    /// Axis-aligned (min, max) corners of everything the shape paints.
    fn bounds(&self) -> (Point, Point);
    fn translate(&mut self, d: Point);
}

/// Distance from `p` to the segment `a`-`b`.
pub fn seg_dist(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (fx, fy) = (p.0 - a.0, p.1 - a.1);
    let t = ((fx * dx + fy * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
    let (ex, ey) = (fx - t * dx, fy - t * dy);
    (ex * ex + ey * ey).sqrt()
}

/// Bounds of two corner points, grown by `pad` on every side.
pub fn corner_bounds(a: Point, b: Point, pad: f32) -> (Point, Point) {
    (
        (a.0.min(b.0) - pad, a.1.min(b.1) - pad),
        (a.0.max(b.0) + pad, a.1.max(b.1) + pad),
    )
}

pub fn offset(p: &mut Point, d: Point) {
    p.0 += d.0;
    p.1 += d.1;
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

    pub fn bounds(&self) -> (Point, Point) {
        self.inner().bounds()
    }

    pub fn translate(&mut self, d: Point) {
        self.inner_mut().translate(d);
    }
}
