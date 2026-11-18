pub mod rectangle;
pub mod stroke;

use rectangle::Rectangle;
use slint::Rgba8Pixel;
use stroke::Stroke;

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
}

pub enum Shape {
    Stroke(Stroke),
    Rectangle(Rectangle),
}

impl Shape {
    pub fn inner(&self) -> &dyn Draw {
        match self {
            Shape::Stroke(s) => s,
            Shape::Rectangle(r) => r,
        }
    }

    pub fn inner_mut(&mut self) -> &mut dyn Draw {
        match self {
            Shape::Stroke(s) => s,
            Shape::Rectangle(r) => r,
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
}
