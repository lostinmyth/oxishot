use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::shapes::{Style, points_bounds};

pub struct Layer {
    pub buf: SharedPixelBuffer<Rgba8Pixel>,
}

impl Layer {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            buf: SharedPixelBuffer::new(w, h),
        }
    }

    pub fn clear(&mut self) {
        self.buf.make_mut_bytes().fill(0);
    }

    pub fn capsule(&mut self, a: (f32, f32), b: (f32, f32), style: Style) {
        let radius = style.radius;
        let color = style.color;

        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let pad = radius + 1.0;
        let x0 = ((a.0.min(b.0) - pad).floor() as i32).max(0);
        let y0 = ((a.1.min(b.1) - pad).floor() as i32).max(0);
        // exclusive upper bounds (loops use x0..x1 / y0..y1)
        let x1 = ((a.0.max(b.0) + pad).ceil() as i32).min(w);
        let y1 = ((a.1.max(b.1) + pad).ceil() as i32).min(h);

        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        let px = self.buf.make_mut_slice();

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

    /// Blends a `gw`x`gh` coverage bitmap (e.g. a glyph) with its top-left at (`x`, `y`).
    pub fn coverage(
        &mut self,
        x: f32,
        y: f32,
        gw: usize,
        gh: usize,
        cov: &[u8],
        color: Rgba8Pixel,
    ) {
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let (ox, oy) = (x.round() as i32, y.round() as i32);
        let (gw_i, gh_i) = (gw as i32, gh as i32);
        // glyph-local ranges that land inside the buffer
        let (gx0, gx1) = ((-ox).max(0), (w - ox).min(gw_i));
        let (gy0, gy1) = ((-oy).max(0), (h - oy).min(gh_i));
        let px = self.buf.make_mut_slice();

        for gy in gy0..gy1 {
            let dst_row = (oy + gy) * w + ox;
            let src_row = gy * gw_i;
            for gx in gx0..gx1 {
                let c = cov[(src_row + gx) as usize];
                if c > 0 {
                    blend_over(&mut px[(dst_row + gx) as usize], color, c as f32 / 255.0);
                }
            }
        }
    }

    /// Like `coverage`, but the bitmap is rotated by `angle` (radians) about
    /// `pivot` and sampled bilinearly.
    pub fn coverage_rotated(
        &mut self,
        (x, y): (f32, f32),
        (gw, gh): (usize, usize),
        cov: &[u8],
        color: Rgba8Pixel,
        pivot: (f32, f32),
        angle: f32,
    ) {
        let (s, k) = angle.sin_cos();
        let rot = |(px, py): (f32, f32)| {
            let (dx, dy) = (px - pivot.0, py - pivot.1);
            (pivot.0 + dx * k - dy * s, pivot.1 + dx * s + dy * k)
        };
        let (gwf, ghf) = (gw as f32, gh as f32);
        let corners = [(x, y), (x + gwf, y), (x + gwf, y + ghf), (x, y + ghf)].map(rot);
        // one extra pixel: bilinear filtering bleeds half a texel outward
        let ((bx0, by0), (bx1, by1)) = points_bounds(corners, 1.0);
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let (x0, y0) = ((bx0.floor() as i32).max(0), (by0.floor() as i32).max(0));
        let (x1, y1) = ((bx1.ceil() as i32).min(w), (by1.ceil() as i32).min(h));

        let (gw_i, gh_i) = (gw as i32, gh as i32);
        let at = |i: i32, j: i32| {
            if i < 0 || j < 0 || i >= gw_i || j >= gh_i {
                0.0
            } else {
                cov[(j * gw_i + i) as usize] as f32
            }
        };
        let px = self.buf.make_mut_slice();

        for py in y0..y1 {
            let row = (py * w) as usize;
            for pxi in x0..x1 {
                // inverse-rotate the pixel center into glyph space; texel
                // centers sit at +0.5, so shift to make them integers
                let (dx, dy) = (pxi as f32 + 0.5 - pivot.0, py as f32 + 0.5 - pivot.1);
                let u = pivot.0 + dx * k + dy * s - x - 0.5;
                let v = pivot.1 + dy * k - dx * s - y - 0.5;
                let (fu, fv) = (u.floor(), v.floor());
                let (tu, tv) = (u - fu, v - fv);
                let (iu, iv) = (fu as i32, fv as i32);
                let top = at(iu, iv) * (1.0 - tu) + at(iu + 1, iv) * tu;
                let bot = at(iu, iv + 1) * (1.0 - tu) + at(iu + 1, iv + 1) * tu;
                let c = top * (1.0 - tv) + bot * tv;
                if c > 0.0 {
                    blend_over(&mut px[row + pxi as usize], color, c / 255.0);
                }
            }
        }
    }

    /// Ellipse outline with center `c`, radii `r` and rotation `angle`
    /// (radians): pixels whose (approximate) distance to the ellipse is
    /// within the stroke radius.
    pub fn ellipse(&mut self, c: (f32, f32), r: (f32, f32), angle: f32, style: Style) {
        let (rx, ry) = r;
        let (s, k) = angle.sin_cos();
        // a flat ellipse is just a line; also avoids dividing by ~0 below
        if rx < 0.5 || ry < 0.5 {
            let (hx, hy) = (rx * k - ry * s, rx * s + ry * k);
            self.capsule((c.0 - hx, c.1 - hy), (c.0 + hx, c.1 + hy), style);
            return;
        }

        let radius = style.radius;
        let color = style.color;

        // half-extents of the rotated ellipse
        let ex = (rx * rx * k * k + ry * ry * s * s).sqrt();
        let ey = (rx * rx * s * s + ry * ry * k * k).sqrt();
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let x0 = ((c.0 - ex - radius - 1.0).floor() as i32).max(0);
        let y0 = ((c.1 - ey - radius - 1.0).floor() as i32).max(0);
        let x1 = ((c.0 + ex + radius + 1.0).ceil() as i32).min(w);
        let y1 = ((c.1 + ey + radius + 1.0).ceil() as i32).min(h);

        let (irx, iry) = (1.0 / rx, 1.0 / ry);
        let px = self.buf.make_mut_slice();

        for y in y0..y1 {
            let row = (y * w) as usize;
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - c.0, y as f32 + 0.5 - c.1);
                // into the ellipse's own frame (rotate by -angle)
                let (fx, fy) = (dx * k + dy * s, dy * k - dx * s);
                // first-order distance: f / |grad f| with f = |p/r| - 1,
                // written as k0 * (k0 - 1) / k1 (accurate near the outline,
                // which is the only place we need it)
                let (ux, uy) = (fx * irx, fy * iry);
                let k0 = (ux * ux + uy * uy).sqrt();
                let (vx, vy) = (ux * irx, uy * iry);
                let k1 = (vx * vx + vy * vy).sqrt();
                if k1 < 1e-6 {
                    continue; // exact center, far from the outline
                }
                let d = (k0 * (k0 - 1.0) / k1).abs();
                let cov = (radius + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend_over(&mut px[row + x as usize], color, cov);
                }
            }
        }
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

/// The final image: `overlay` blended over a copy of `base` (same size).
pub fn flatten(
    base: &SharedPixelBuffer<Rgba8Pixel>,
    overlay: &SharedPixelBuffer<Rgba8Pixel>,
) -> SharedPixelBuffer<Rgba8Pixel> {
    let mut out = base.clone();
    for (d, &s) in out.make_mut_slice().iter_mut().zip(overlay.as_slice()) {
        match s.a {
            0 => {}
            255 => *d = s,
            _ => blend_over(d, s, 1.0),
        }
    }
    out
}
