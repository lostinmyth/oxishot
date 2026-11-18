use std::sync::LazyLock;

use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};

use crate::{
    raster::Layer,
    shapes::{Draw, Point, Style},
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
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: self.pos.0,
            y: self.pos.1,
            ..Default::default()
        });
        layout.append(&[&*FONT], &TextStyle::new(&self.text, self.size, 0));

        for g in layout.glyphs() {
            if g.width == 0 || g.height == 0 {
                continue; // whitespace
            }
            let (_, cov) = FONT.rasterize_config(g.key);
            layer.coverage(g.x, g.y, g.width, g.height, &cov, self.style.color);
        }
    }
}
