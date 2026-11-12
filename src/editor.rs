use crate::{raster::Layer, shapes::Shape};

use slint::{Rgba8Pixel, SharedPixelBuffer};

pub struct Editor {
    pub layer: Layer,
    pub history: Vec<Shape>,
    pub redo: Vec<Shape>,
    pub current: Option<Shape>,
    pub live_path: String,
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            layer: Layer::new(width, height),
            history: Vec::new(),
            redo: Vec::new(),
            current: None,
            live_path: String::new(),
        }
    }

    pub fn rasterize(&mut self, shape: &Shape) {
        match shape {
            Shape::Stroke(stroke) => {
                let pts = &stroke.points;
                if pts.len() == 1 {
                    self.capsule(pts[0], pts[0], stroke.radius, stroke.color);
                }
                for w in pts.windows(2) {
                    self.capsule(w[0], w[1], stroke.radius, stroke.color);
                }
            }
            Shape::Rectangle(rect) => {
                let (x0, y0) = rect.a;
                let (x1, y1) = rect.b;
                let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
                for i in 0..4 {
                    self.capsule(corners[i], corners[(i + 1) % 4], rect.radius, rect.color);
                }
            }
        }
    }

    fn capsule(&mut self, a: (f32, f32), b: (f32, f32), radius: f32, color: Rgba8Pixel) {
        self.layer.capsule(a, b, radius, color);
    }

    pub fn rebuild(&mut self) {
        self.layer.clear();
        let strokes = std::mem::take(&mut self.history);
        for s in &strokes {
            self.rasterize(s);
        }
        self.history = strokes;
    }
}
