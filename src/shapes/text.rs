//! Text shapes, laid out with parley and rasterized with swash: the same
//! stack (and the same system font collection) Slint's TextInput uses for
//! the live preview, so the committed raster matches it.

use std::{borrow::Cow, cell::RefCell};

use parley::{
    FontContext, FontFamily, FontFamilyName, FontWeight, Layout, LayoutContext,
    PositionedLayoutItem, StyleProperty,
    fontique::{FamilyId, GenericFamily},
};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source, image::Content},
    zeno::{Format, Vector},
};

use crate::{
    raster::Layer,
    shapes::{Draw, Frame, Point, Scale, Style, offset, rotated},
};

/// Same fallbacks Slint appends after a TextInput's font-family.
const FALLBACKS: [GenericFamily; 2] = [GenericFamily::SansSerif, GenericFamily::SystemUi];

/// Which system font a text uses.
#[derive(Clone, Copy, PartialEq)]
pub struct Font {
    pub family: FamilyId,
    /// CSS weight (400 regular, 700 bold).
    pub weight: f32,
    /// px per em, same number as Slint's font-size.
    pub size: f32,
}

struct Engine {
    fonts: FontContext,
    layout: LayoutContext<()>,
    scale: ScaleContext,
}

thread_local! {
    // the system collection is scanned once, on first use
    static ENGINE: RefCell<Engine> = RefCell::new(Engine {
        fonts: FontContext::new(),
        layout: LayoutContext::new(),
        scale: ScaleContext::new(),
    });
}

/// Every installed font family, sorted for display.
pub fn families() -> Vec<String> {
    ENGINE.with_borrow_mut(|e| {
        let mut v: Vec<String> = e.fonts.collection.family_names().map(str::to_owned).collect();
        v.sort_by_cached_key(|s| s.to_lowercase());
        v.dedup();
        v
    })
}

pub fn family_id(name: &str) -> Option<FamilyId> {
    ENGINE.with_borrow_mut(|e| e.fonts.collection.family_by_name(name).map(|f| f.id()))
}

pub fn family_name(id: FamilyId) -> String {
    ENGINE.with_borrow_mut(|e| e.fonts.collection.family(id).map_or_else(String::new, |f| f.name().to_owned()))
}

/// The family Slint shows when none is set: the system's sans-serif.
pub fn default_family() -> FamilyId {
    ENGINE.with_borrow_mut(|e| {
        let c = &mut e.fonts.collection;
        let generic = FALLBACKS.into_iter().find_map(|g| c.generic_families(g).next());
        generic
            .or_else(|| {
                let first = c.family_names().next()?.to_owned();
                c.family_by_name(&first).map(|f| f.id())
            })
            .expect("no fonts installed")
    })
}

/// The weights `family` has upright faces for, ascending. A variable font
/// offers every hundred its weight axis spans.
pub fn weights(family: FamilyId) -> Vec<u16> {
    ENGINE.with_borrow_mut(|e| {
        let Some(f) = e.fonts.collection.family(family) else {
            return vec![400];
        };
        let mut v = Vec::new();
        for font in f.fonts() {
            if font.style() != parley::fontique::FontStyle::Normal {
                continue;
            }
            let axis = font.axes().iter().find(|a| a.tag.to_be_bytes() == *b"wght");
            match axis {
                Some(a) if font.has_weight_axis() => {
                    let (lo, hi) = ((a.min / 100.0).ceil() as u16, (a.max / 100.0).floor() as u16);
                    v.extend((lo..=hi).map(|w| w * 100));
                }
                _ => v.push(font.weight().value().round() as u16),
            }
        }
        v.sort_unstable();
        v.dedup();
        if v.is_empty() {
            v.push(400);
        }
        v
    })
}

/// The entry of `weights` closest to `w`: picking one the family has keeps
/// the preview from synthesizing a bold the raster wouldn't.
pub fn nearest_weight(weights: &[u16], w: f32) -> u16 {
    *weights
        .iter()
        .min_by_key(|&&x| (x as f32 - w).abs() as u32)
        .unwrap_or(&400)
}

#[derive(Clone)]
pub struct Text {
    pub pos: Point, // top-left of the line box (the TextInput's x/y)
    pub text: String,
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
        let color = self.style.color;
        ENGINE.with_borrow_mut(|e| {
            for line in layout.lines() {
                for item in line.items() {
                    let PositionedLayoutItem::GlyphRun(run) = item else {
                        continue;
                    };
                    let r = run.run();
                    let font = r.font();
                    let Some(face) = FontRef::from_index(font.data.data(), font.index as usize) else {
                        continue;
                    };
                    let mut scaler = (e.scale.builder(face))
                        .size(r.font_size())
                        .normalized_coords(r.normalized_coords())
                        .hint(false)
                        .build();
                    for g in run.positioned_glyphs() {
                        let (x, y) = (self.pos.0 + g.x, self.pos.1 + g.y);
                        // swash's offset is y-up: render at the pixel origin
                        // below the baseline and lift the glyph by the remainder
                        let (ox, oy) = (x.floor(), y.ceil());
                        let Some(img) = Render::new(&[Source::Outline])
                            .format(Format::Alpha)
                            .offset(Vector::new(x - ox, oy - y))
                            .render(&mut scaler, g.id as u16)
                        else {
                            continue;
                        };
                        let p = img.placement;
                        if p.width == 0 || p.height == 0 || img.content != Content::Mask {
                            continue; // whitespace, or color glyphs (emoji) we can't tint
                        }
                        let pos = (ox + p.left as f32, oy - p.top as f32);
                        let size = (p.width as usize, p.height as usize);
                        if self.angle == 0.0 {
                            layer.coverage(pos.0, pos.1, size.0, size.1, &img.data, color);
                        } else {
                            layer.coverage_rotated(pos, size, &img.data, color, self.pos, self.angle);
                        }
                    }
                }
            }
        });
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
        self.style.font.size *= (s.factors.0 * s.factors.1).sqrt();
    }
}

impl Text {
    /// The line box before rotation.
    fn line_box(&self) -> (Point, Point) {
        let l = self.layout();
        let (x, y) = self.pos;
        ((x, y), (x + l.width(), y + l.height()))
    }

    fn layout(&self) -> Layout<()> {
        let Font { family, weight, size } = self.style.font;
        ENGINE.with_borrow_mut(|e| {
            let name = e.fonts.collection.family(family).map(|f| f.name().to_owned());
            let families: Vec<_> = (name.into_iter().map(|n| FontFamilyName::Named(n.into())))
                .chain(FALLBACKS.map(FontFamilyName::Generic))
                .collect();
            let mut b = e.layout.ranged_builder(&mut e.fonts, &self.text, 1.0, false);
            b.push_default(FontFamily::List(Cow::Borrowed(&families)));
            b.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
            b.push_default(StyleProperty::FontSize(size));
            let mut layout = b.build(&self.text);
            layout.break_all_lines(None);
            layout
        })
    }
}

