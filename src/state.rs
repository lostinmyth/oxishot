use slint::{Rgba8Pixel, SharedPixelBuffer};

pub struct Stroke {
    pub points: Vec<(f32, f32)>,
    pub color: Rgba8Pixel,
    pub radius: f32,
}

pub struct Editor {
    pub layer: SharedPixelBuffer<Rgba8Pixel>,
    pub history: Vec<Stroke>,
    pub redo: Vec<Stroke>,
    pub current: Option<Stroke>,
    pub live_path: String,
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            layer: SharedPixelBuffer::new(width, height),
            history: Vec::new(),
            redo: Vec::new(),
            current: None,
            live_path: String::new(),
        }
    }

    pub fn rasterize(&mut self, stroke: &Stroke) {
        let pts = &stroke.points;
        if pts.len() == 1 {
            self.capsule(pts[0], pts[0], stroke.radius, stroke.color);
        }
        for w in pts.windows(2) {
            self.capsule(w[0], w[1], stroke.radius, stroke.color);
        }
    }

    fn capsule(&mut self, a: (f32, f32), b: (f32, f32), radius: f32, color: Rgba8Pixel) {
        let (w, h) = (self.layer.width() as i32, self.layer.height() as i32);
        let pad = radius + 1.0;
        let x0 = ((a.0.min(b.0) - pad).floor() as i32).max(0);
        let y0 = ((a.1.min(b.1) - pad).floor() as i32).max(0);
        let x1 = ((a.0.max(b.0) + pad).ceil() as i32).min(w - 1);
        let y1 = ((a.1.max(b.1) + pad).ceil() as i32).max(h - 1);

        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        let px = self.layer.make_mut_slice();

        for y in y0..y1 {
            let row = (y * w) as usize;
            for x in x0..x1 {
                let (fx, fy) = (x as f32 + 0.5 - a.0, y as f32 + 0.5 - a.1);
                let t = ((fx * dx + fy * dy) / len2).clamp(0.0, 1.0);
                let (ex, ey) = (fx - t * dx, fy - t * dy);
                let cov = (radius + 0.5 - (ex * ex + ey * ey).sqrt()).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend_over(&mut px[row + x as usize], color, cov);
                }
            }
        }
    }

    pub fn rebuild(&mut self) {
        self.layer.make_mut_bytes().fill(0);
        let strokes = std::mem::take(&mut self.history);
        for s in &strokes {
            self.rasterize(s);
        }
        self.history = strokes;
    }
}

fn blend_over(dst: &mut Rgba8Pixel, src: Rgba8Pixel, cov: f32) {
    let sa = src.a as f32 / 255.0 * cov;
    let da = dst.a as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        return;
    }
    let mix = |s: u8, d: u8| ((s as f32 * sa + d as f32 * da * (1.0 - sa)) / oa) as u8;
    *dst = Rgba8Pixel {
        r: mix(src.r, dst.r),
        g: mix(src.g, dst.g),
        b: mix(src.b, dst.b),
        a: (oa * 255.0) as u8,
    }
}
