pub mod rectangle;
pub mod stroke;

use rectangle::Rectangle;
use stroke::Stroke;

pub enum Shape {
    Stroke(Stroke),
    Rectangle(Rectangle),
}

impl Shape {
    pub fn write_preview(&self, out: &mut String) {
        use std::fmt::Write;
        out.clear();
        match self {
            Shape::Rectangle(r) => {
                let ((x0, y0), (x1, y1)) = (r.a, r.b);
                let _ = write!(out, "M {x0} {y0} L {x1} {y0} L {x1} {y1} L {x0} {y1} Z");
            }
            _ => {}
        }
    }
}
