use std::sync::LazyLock;

use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};

use crate::{
    raster::Layer,
    shapes::{Draw, Point, Style, offset},
};

/// Same file the Slint preview imports, so the committed raster matches it.
pub static FONT: LazyLock<Font> = LazyLock::new(|| {
    Font::from_bytes(
        include_bytes!("../assets/fonts/DejaVuSans.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("bundled font is valid")
});

pub struct Text {
    pub pos: Point, // top-left of the line box (the TextInput's x/y)
    pub text: String,
    pub size: f32, // px per em, same number as Slint's font-size
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
            layer.coverage(g.x, g.y, g.width, g.height, &cov, self.style.color);
        }
    }

    // text is small and has gaps between glyphs, so its whole box counts
    fn hit(&self, p: Point, tol: f32) -> bool {
        let ((x0, y0), (x1, y1)) = self.bounds();
        p.0 >= x0 - tol && p.0 <= x1 + tol && p.1 >= y0 - tol && p.1 <= y1 + tol
    }

    fn bounds(&self) -> (Point, Point) {
        let layout = self.layout();
        let right = layout
            .glyphs()
            .last()
            .map_or(self.pos.0, |g| g.x + g.width as f32);
        (self.pos, (right, self.pos.1 + layout.height()))
    }

    fn translate(&mut self, d: Point) {
        offset(&mut self.pos, d);
    }
}

impl Text {
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
