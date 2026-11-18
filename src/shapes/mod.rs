pub mod arrow;
pub mod blur;
pub mod circle;
pub mod rectangle;
pub mod stroke;
pub mod text;

use arrow::Arrow;
use blur::Blur;
use circle::Circle;
use rectangle::Rectangle;
use slint::Rgba8Pixel;
use stroke::Stroke;
use text::Text;

use crate::raster::Layer;

pub type Point = (f32, f32);

#[derive(Clone, Copy, PartialEq)]
pub struct Style {
    /// Stroke color (text: glyph color).
    pub color: Rgba8Pixel,
    /// Stroke half-width; unused by text.
    pub radius: f32,
    /// Interior color; only rectangles and circles have one.
    pub fill: Option<Rgba8Pixel>,
    /// Only text uses it.
    pub font: text::Font,
    /// Blur strength (px, roughly the Gaussian's standard deviation); only blurs use it.
    pub blur: f32,
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
    /// Applies `s`. Uneven factors must run along the shape's own frame
    /// axes (`frame().angle`); any other scale has to be uniform.
    fn scale(&mut self, s: &Scale);
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

/// If `angle` is a whole number of quarter turns, whether that number is odd.
pub fn quarter_turns(angle: f32) -> Option<bool> {
    let q = angle / std::f32::consts::FRAC_PI_2;
    let n = q.round();
    ((q - n).abs() < 1e-4).then(|| n.rem_euclid(2.0) == 1.0)
}

/// `p` moved by under half a pixel so a stroke of half-width `r` centered on
/// it starts on a pixel boundary (pointer positions are fractional, and a
/// line between pixels smears into two half-covered rows).
pub fn snap_point(p: Point, r: f32) -> Point {
    ((p.0 - r).round() + r, (p.1 - r).round() + r)
}

/// An outline (center, half-size, angle) stroked with half-width `r`: if it's
/// axis-aligned, re-expressed unrotated with its outer stroke edges moved
/// onto pixel boundaries so straight sides come out fully crisp.
pub fn snap_outline(f: Frame, r: f32) -> Frame {
    let Some(odd) = quarter_turns(f.angle) else {
        return f;
    };
    // a half turn maps the box onto itself; a quarter turn swaps its sides
    let (hx, hy) = if odd { (f.half.1, f.half.0) } else { f.half };
    let lo = |v: f32| (v - r).round() + r;
    let hi = |v: f32, l: f32| ((v + r).round() - r).max(l);
    let (x0, y0) = (lo(f.center.0 - hx), lo(f.center.1 - hy));
    let (x1, y1) = (hi(f.center.0 + hx, x0), hi(f.center.1 + hy, y0));
    Frame::aligned(((x0, y0), (x1, y1)))
}

/// A stretch by `factors` along axes turned `angle` radians, about `anchor`.
#[derive(Clone, Copy)]
pub struct Scale {
    pub anchor: Point,
    pub angle: f32,
    pub factors: Point,
}

impl Scale {
    /// `p` relative to the anchor, measured along the scale's axes.
    pub fn local(&self, p: Point) -> Point {
        rotated(
            (p.0 - self.anchor.0, p.1 - self.anchor.1),
            (0.0, 0.0),
            (-self.angle).sin_cos(),
        )
    }

    pub fn apply(&self, p: Point) -> Point {
        let (u, v) = self.local(p);
        let (x, y) = rotated(
            (u * self.factors.0, v * self.factors.1),
            (0.0, 0.0),
            self.angle.sin_cos(),
        );
        (self.anchor.0 + x, self.anchor.1 + y)
    }

    pub fn inverse(&self) -> Self {
        Self {
            factors: (1.0 / self.factors.0, 1.0 / self.factors.1),
            ..*self
        }
    }
}

/// Scales the box spanned by corners `a` and `b` (stored unrotated about
/// their midpoint): the midpoint moves with `s`, the corners stretch around it.
pub fn scale_box(a: &mut Point, b: &mut Point, s: &Scale) {
    let c = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let n = s.apply(c);
    let (fx, fy) = s.factors;
    for q in [a, b] {
        *q = (n.0 + (q.0 - c.0) * fx, n.1 + (q.1 - c.1) * fy);
    }
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

    /// The frame after scaling its owner by `s` (same axis rule as `Draw::scale`).
    pub fn scaled(&self, s: &Scale) -> Self {
        Self {
            center: s.apply(self.center),
            half: (self.half.0 * s.factors.0, self.half.1 * s.factors.1),
            angle: self.angle,
        }
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

#[derive(Clone)]
pub enum Shape {
    Stroke(Stroke),
    Rectangle(Rectangle),
    Circle(Circle),
    Arrow(Arrow),
    Text(Text),
    Blur(Blur),
}

impl Shape {
    pub fn inner(&self) -> &dyn Draw {
        match self {
            Shape::Stroke(s) => s,
            Shape::Rectangle(r) => r,
            Shape::Circle(c) => c,
            Shape::Arrow(a) => a,
            Shape::Text(t) => t,
            Shape::Blur(b) => b,
        }
    }

    pub fn inner_mut(&mut self) -> &mut dyn Draw {
        match self {
            Shape::Stroke(s) => s,
            Shape::Rectangle(r) => r,
            Shape::Circle(c) => c,
            Shape::Arrow(a) => a,
            Shape::Text(t) => t,
            Shape::Blur(b) => b,
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

    pub fn scale(&mut self, s: &Scale) {
        self.inner_mut().scale(s);
    }

    pub fn style(&self) -> &Style {
        match self {
            Shape::Stroke(s) => &s.style,
            Shape::Rectangle(r) => &r.style,
            Shape::Circle(c) => &c.style,
            Shape::Arrow(a) => &a.style,
            Shape::Text(t) => &t.style,
            Shape::Blur(b) => &b.style,
        }
    }

    pub fn style_mut(&mut self) -> &mut Style {
        match self {
            Shape::Stroke(s) => &mut s.style,
            Shape::Rectangle(r) => &mut r.style,
            Shape::Circle(c) => &mut c.style,
            Shape::Arrow(a) => &mut a.style,
            Shape::Text(t) => &mut t.style,
            Shape::Blur(b) => &mut b.style,
        }
    }

    /// Whether `Style::fill` paints anything for this shape.
    pub fn fillable(&self) -> bool {
        matches!(self, Shape::Rectangle(_) | Shape::Circle(_))
    }

    /// Whether `Style::radius` (stroke width) applies to this shape.
    pub fn stroked(&self) -> bool {
        !matches!(self, Shape::Text(_) | Shape::Blur(_))
    }

    /// Whether `Style::color` paints anything for this shape.
    pub fn colored(&self) -> bool {
        !matches!(self, Shape::Blur(_))
    }
}
