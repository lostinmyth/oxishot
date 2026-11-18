use std::sync::LazyLock;

use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};

use crate::{
    raster::Layer,
    shapes::{Draw, Frame, Point, Scale, Style, offset, rotated},
};

/// Same file the Slint preview imports, so the committed raster matches it.
pub static FONT: LazyLock<Font> = LazyLock::new(|| {
    Font::from_bytes(
        include_bytes!("../assets/fonts/DejaVuSans.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("bundled font is valid")
});

#[derive(Clone)]
pub struct Text {
    pub pos: Point, // top-left of the line box (the TextInput's x/y)
    pub text: String,
    pub size: f32, // px per em, same number as Slint's font-size
    /// Rotation about `pos`, in radians.
    pub angle: f32,
    pub style: Style,
}

impl Draw for Text {
    // Slint's TextInput is the live preview; there is no path to draw.
    fn write_preview(&self, out: &mut String) {
        out.clear();
    }

    fn extend(&mut self, _: Point, _: &mut String) -> bool {
        false
    }

    fn rasterize(&self, layer: &mut Layer) {
        let layout = self.layout();
        for g in layout.glyphs() {
            if g.width == 0 || g.height == 0 {
                continue; // whitespace
            }
            let (_, cov) = FONT.rasterize_config(g.key);
            if self.angle == 0.0 {
                layer.coverage(g.x, g.y, g.width, g.height, &cov, self.style.color);
            } else {
                let (pos, size) = ((g.x, g.y), (g.width, g.height));
                layer.coverage_rotated(pos, size, &cov, self.style.color, self.pos, self.angle);
            }
        }
    }

    // text is small and has gaps between glyphs, so its whole box counts
    fn hit(&self, p: Point, tol: f32) -> bool {
        self.frame().contains(p, tol)
    }

    fn frame(&self) -> Frame {
        // the line box turns about `pos`, so its center moves with it
        let f = Frame::aligned(self.line_box());
        Frame {
            center: rotated(f.center, self.pos, self.angle.sin_cos()),
            angle: self.angle,
            ..f
        }
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.pos, d);
    }

    fn rotate(&mut self, c: Point, angle: f32) {
        self.pos = rotated(self.pos, c, angle.sin_cos());
        self.angle += angle;
    }

    // glyphs can't stretch, so text only scales uniformly (the editor
    // ensures that); the mean keeps this sane if it's ever called otherwise
    fn scale(&mut self, s: &Scale) {
        self.pos = s.apply(self.pos);
        self.size *= (s.factors.0 * s.factors.1).sqrt();
    }
}

impl Text {
    /// The line box before rotation.
    fn line_box(&self) -> (Point, Point) {
        let layout = self.layout();
        let right = layout
            .glyphs()
            .last()
            .map_or(self.pos.0, |g| g.x + g.width as f32);
        (self.pos, (right, self.pos.1 + layout.height()))
    }

    fn layout(&self) -> Layout {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: self.pos.0,
            y: self.pos.1,
            ..Default::default()
        });
        layout.append(&[&*FONT], &TextStyle::new(&self.text, self.size, 0));
        layout
    }
}
