use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::shapes::{Frame, Style, points_bounds};

pub struct Layer {
    pub buf: SharedPixelBuffer<Rgba8Pixel>,
    /// The screenshot under the layer, which blurs read (shared, not copied).
    base: SharedPixelBuffer<Rgba8Pixel>,
}

/// Box passes per axis in `blur_box`; three already look Gaussian.
const BLUR_PASSES: usize = 3;

impl Layer {
    /// An empty layer the size of `base`, which it's drawn over.
    pub fn new(base: SharedPixelBuffer<Rgba8Pixel>) -> Self {
        Self {
            buf: SharedPixelBuffer::new(base.width(), base.height()),
            base,
        }
    }

    /// An empty layer over the same screenshot.
    pub fn blank(&self) -> Self {
        Self::new(self.base.clone())
    }

    /// An empty layer over this one and its screenshot, so blurs drawn on
    /// it see both.
    pub fn above(&self) -> Self {
        Self::new(flatten(&self.base, &self.buf))
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

    /// Rectangle outline with center `c`, half-size `half` and rotation
    /// `angle` (radians): the band between the box grown and shrunk by the
    /// stroke radius, so corners stay square instead of rounding off.
    pub fn box_outline(&mut self, c: (f32, f32), half: (f32, f32), angle: f32, style: Style) {
        let r = style.radius;
        let color = style.color;
        let outer = (half.0 + r, half.1 + r);
        let inner = (half.0 - r, half.1 - r);
        let (s, k) = angle.sin_cos();

        let ((bx0, by0), (bx1, by1)) = Frame { center: c, half: outer, angle }.aabb();
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let x0 = ((bx0 - 1.0).floor() as i32).max(0);
        let y0 = ((by0 - 1.0).floor() as i32).max(0);
        let x1 = ((bx1 + 1.0).ceil() as i32).min(w);
        let y1 = ((by1 + 1.0).ceil() as i32).min(h);

        // how much of the pixel span [x - 0.5, x + 0.5] lies in [-h, h]
        let span = |x: f32, h: f32| (f32::min(x + 0.5, h) - f32::max(x - 0.5, -h)).max(0.0);
        let px = self.buf.make_mut_slice();

        for y in y0..y1 {
            let row = (y * w) as usize;
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - c.0, y as f32 + 0.5 - c.1);
                // into the box's own frame (rotate by -angle)
                let (fx, fy) = (dx * k + dy * s, dy * k - dx * s);
                // pixel area inside the outer box minus inside the inner one;
                // per-axis overlap is exact while the box is axis-aligned
                let cov = span(fx, outer.0) * span(fy, outer.1)
                    - span(fx, inner.0) * span(fy, inner.1);
                if cov > 0.0 {
                    blend_over(&mut px[row + x as usize], color, cov.min(1.0));
                }
            }
        }
    }

    /// Solid rectangle with center `c`, half-size `half` and rotation `angle`.
    pub fn box_fill(&mut self, c: (f32, f32), half: (f32, f32), angle: f32, color: Rgba8Pixel) {
        let (s, k) = angle.sin_cos();
        let ((bx0, by0), (bx1, by1)) = Frame { center: c, half, angle }.aabb();
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let x0 = ((bx0 - 1.0).floor() as i32).max(0);
        let y0 = ((by0 - 1.0).floor() as i32).max(0);
        let x1 = ((bx1 + 1.0).ceil() as i32).min(w);
        let y1 = ((by1 + 1.0).ceil() as i32).min(h);

        // same per-axis pixel overlap as box_outline
        let span = |x: f32, h: f32| (f32::min(x + 0.5, h) - f32::max(x - 0.5, -h)).max(0.0);
        let px = self.buf.make_mut_slice();

        for y in y0..y1 {
            let row = (y * w) as usize;
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - c.0, y as f32 + 0.5 - c.1);
                let (fx, fy) = (dx * k + dy * s, dy * k - dx * s);
                let cov = span(fx, half.0) * span(fy, half.1);
                if cov > 0.0 {
                    blend_over(&mut px[row + x as usize], color, cov.min(1.0));
                }
            }
        }
    }

    /// Solid ellipse with center `c`, radii `r` and rotation `angle`.
    pub fn ellipse_fill(&mut self, c: (f32, f32), r: (f32, f32), angle: f32, color: Rgba8Pixel) {
        let (rx, ry) = r;
        if rx < 0.5 || ry < 0.5 {
            return; // no area; the outline covers it
        }
        let (s, k) = angle.sin_cos();
        let ex = (rx * rx * k * k + ry * ry * s * s).sqrt();
        let ey = (rx * rx * s * s + ry * ry * k * k).sqrt();
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let x0 = ((c.0 - ex - 1.0).floor() as i32).max(0);
        let y0 = ((c.1 - ey - 1.0).floor() as i32).max(0);
        let x1 = ((c.0 + ex + 1.0).ceil() as i32).min(w);
        let y1 = ((c.1 + ey + 1.0).ceil() as i32).min(h);

        let (irx, iry) = (1.0 / rx, 1.0 / ry);
        let px = self.buf.make_mut_slice();

        for y in y0..y1 {
            let row = (y * w) as usize;
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - c.0, y as f32 + 0.5 - c.1);
                let (fx, fy) = (dx * k + dy * s, dy * k - dx * s);
                let (ux, uy) = (fx * irx, fy * iry);
                let k0 = (ux * ux + uy * uy).sqrt();
                let (vx, vy) = (ux * irx, uy * iry);
                let k1 = (vx * vx + vy * vy).sqrt();
                // signed version of the distance in `ellipse` (negative inside)
                let cov = if k1 < 1e-6 {
                    1.0
                } else {
                    (0.5 - k0 * (k0 - 1.0) / k1).clamp(0.0, 1.0)
                };
                if cov > 0.0 {
                    blend_over(&mut px[row + x as usize], color, cov);
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

impl Layer {
    /// Blurs what shows inside the box `f` (this layer over the screenshot)
    /// and paints the result back over it, so earlier shapes get blurred too.
    /// `sigma` is about the standard deviation of the Gaussian it imitates:
    /// a few box blurs in a row, each a sliding sum, so the cost doesn't
    /// grow with the strength.
    pub fn blur_box(&mut self, f: Frame, sigma: f32) {
        let (w, h) = (self.buf.width() as i32, self.buf.height() as i32);
        let ((bx0, by0), (bx1, by1)) = f.aabb();
        let x0 = ((bx0 - 1.0).floor() as i32).max(0);
        let y0 = ((by0 - 1.0).floor() as i32).max(0);
        let x1 = ((bx1 + 1.0).ceil() as i32).min(w);
        let y1 = ((by1 + 1.0).ceil() as i32).min(h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }

        // n boxes of width d have variance n(d² - 1)/12
        let d = (12.0 * sigma.max(0.0).powi(2) / BLUR_PASSES as f32 + 1.0).sqrt();
        let r = ((d - 1.0) * 0.5).round() as usize;
        // every pass reaches r px further out
        let reach = (BLUR_PASSES * r) as i32;
        let (sx0, sy0) = ((x0 - reach).max(0), (y0 - reach).max(0));
        let (sx1, sy1) = ((x1 + reach).min(w), (y1 + reach).min(h));
        let (sw, sh) = ((sx1 - sx0) as usize, (sy1 - sy0) as usize);
        let (ow, oh) = ((x1 - x0) as usize, (y1 - y0) as usize);

        // what's visible there now
        let mut src = Vec::with_capacity(sw * sh);
        let (top, base) = (self.buf.as_slice(), self.base.as_slice());
        for y in sy0..sy1 {
            let row = (y * w) as usize;
            for x in sx0..sx1 {
                let i = row + x as usize;
                let mut p = base[i];
                match top[i].a {
                    0 => {}
                    255 => p = top[i],
                    _ => blend_over(&mut p, top[i], 1.0),
                }
                src.push(p);
            }
        }

        // both axes run as row passes: each one writes its result transposed,
        // keeping only the part the next step reads
        let mut cols = vec![Rgba8Pixel::default(); ow * sh];
        blur_rows_t(&src, sw, r, (x0 - sx0) as usize, &mut cols);
        let mut out = vec![Rgba8Pixel::default(); oh * ow];
        blur_rows_t(&cols, sh, r, (y0 - sy0) as usize, &mut out);

        // painted with box_fill's coverage, so turned edges stay smooth
        let (s, k) = f.angle.sin_cos();
        let span = |x: f32, h: f32| (f32::min(x + 0.5, h) - f32::max(x - 0.5, -h)).max(0.0);
        let px = self.buf.make_mut_slice();
        for y in y0..y1 {
            let row = (y * w) as usize;
            let orow = (y - y0) as usize * ow;
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - f.center.0, y as f32 + 0.5 - f.center.1);
                let (fx, fy) = (dx * k + dy * s, dy * k - dx * s);
                let cov = span(fx, f.half.0) * span(fy, f.half.1);
                if cov > 0.0 {
                    let p = out[orow + (x - x0) as usize];
                    blend_over(&mut px[row + x as usize], p, cov.min(1.0));
                }
            }
        }
    }
}

/// Box-blurs each `w`-pixel row of `src` `BLUR_PASSES` times with radius
/// `r` (the edge pixels repeat outward), then writes columns `lo..` of the
/// result into `dst` transposed: column `lo + i` becomes row `i`, so `dst`
/// is as wide as `src` is tall, and the next pass reads it row by row.
fn blur_rows_t(src: &[Rgba8Pixel], w: usize, r: usize, lo: usize, dst: &mut [Rgba8Pixel]) {
    let rows = src.len() / w;
    let n = dst.len() / rows; // columns kept
    let (mut a, mut b) = (vec![Rgba8Pixel::default(); w], vec![Rgba8Pixel::default(); w]);
    for (y, line) in src.chunks_exact(w).enumerate() {
        a.copy_from_slice(line);
        for _ in 0..BLUR_PASSES {
            box_line(&a, &mut b, r);
            std::mem::swap(&mut a, &mut b);
        }
        for (i, &p) in a[lo..lo + n].iter().enumerate() {
            dst[i * rows + y] = p;
        }
    }
}

/// One box blur of radius `r` along a line: a running sum per channel,
/// moved one pixel at a time, so each output costs an add and a subtract.
fn box_line(src: &[Rgba8Pixel], dst: &mut [Rgba8Pixel], r: usize) {
    let last = src.len() as isize - 1;
    let at = |i: isize| src[i.clamp(0, last) as usize];
    let ri = r as isize;
    let d = 2 * r as u32 + 1;
    // dividing by d is a multiply by a 32.32 reciprocal: rounding it up
    // keeps the quotient exact for sums this small (at most 255 * d)
    let m = (1u64 << 32).div_ceil(d as u64);
    let div = |s: u32| (((s + d / 2) as u64 * m) >> 32) as u8;

    let mut sum = [0u32; 4];
    for i in -ri..=ri {
        let p = at(i);
        sum[0] += p.r as u32;
        sum[1] += p.g as u32;
        sum[2] += p.b as u32;
        sum[3] += p.a as u32;
    }
    for (x, out) in dst.iter_mut().enumerate() {
        *out = Rgba8Pixel {
            r: div(sum[0]),
            g: div(sum[1]),
            b: div(sum[2]),
            a: div(sum[3]),
        };
        let (add, sub) = (at(x as isize + ri + 1), at(x as isize - ri));
        sum[0] = sum[0] + add.r as u32 - sub.r as u32;
        sum[1] = sum[1] + add.g as u32 - sub.g as u32;
        sum[2] = sum[2] + add.b as u32 - sub.b as u32;
        sum[3] = sum[3] + add.a as u32 - sub.a as u32;
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
