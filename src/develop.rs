//! Non-destructive JPEG develop — CPU preview + export bake.

use crate::catalog::DevelopParams;
use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};
use std::path::Path;

/// Load JPEG (or PNG) from disk.
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    image::open(path).with_context(|| format!("open image {}", path.display()))
}

/// Downscale for interactive preview (longest edge).
pub fn for_preview(img: &DynamicImage, max_edge: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let edge = w.max(h);
    if edge <= max_edge {
        return img.clone();
    }
    let scale = max_edge as f32 / edge as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    img.resize(nw, nh, image::imageops::FilterType::Triangle)
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Apply develop params. Operates in approximate display-referred space.
pub fn apply_develop(img: &DynamicImage, p: &DevelopParams) -> RgbImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    // Exposure as stops → multiply in linear
    let exp_mul = 2f32.powf(p.exposure);
    // Contrast around mid-gray
    let contrast = 1.0 + p.contrast / 100.0;
    // Highlights / shadows soft pulls (−100…100)
    let hi = p.highlights / 100.0;
    let sh = p.shadows / 100.0;
    // WB: temp cool←→warm, tint green←→magenta (simple channel biases)
    let temp = p.temp / 100.0;
    let tint = p.tint / 100.0;
    let sat = 1.0 + p.saturation / 100.0;
    let clarity = p.clarity / 100.0;

    // Optional cheap clarity: unsharp-ish midtone contrast via local luminance vs blurred.
    // For MVP speed we approximate clarity as midtone S-curve strength without a full blur pass
    // when |clarity| is small; for nonzero we do a box blur of luminance.
    let use_clarity = clarity.abs() > 0.01;
    let mut lum_blur: Option<Vec<f32>> = None;
    if use_clarity {
        let mut lum = vec![0f32; (w * h) as usize];
        for (i, px) in rgb.pixels().enumerate() {
            let r = px[0] as f32 / 255.0;
            let g = px[1] as f32 / 255.0;
            let b = px[2] as f32 / 255.0;
            lum[i] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        }
        lum_blur = Some(box_blur_f32(&lum, w as usize, h as usize, 3));
    }

    let mut out: RgbImage = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let px = rgb.get_pixel(x, y);
            let mut r = px[0] as f32 / 255.0;
            let mut g = px[1] as f32 / 255.0;
            let mut b = px[2] as f32 / 255.0;

            // WB biases in sRGB (cheap)
            r = clamp01(r + temp * 0.08 - tint * 0.04);
            g = clamp01(g + tint * 0.06);
            b = clamp01(b - temp * 0.08 - tint * 0.02);

            // Linear exposure
            let rl = srgb_to_linear(r) * exp_mul;
            let gl = srgb_to_linear(g) * exp_mul;
            let bl = srgb_to_linear(b) * exp_mul;

            r = linear_to_srgb(rl.max(0.0));
            g = linear_to_srgb(gl.max(0.0));
            b = linear_to_srgb(bl.max(0.0));

            // Contrast
            r = clamp01((r - 0.5) * contrast + 0.5);
            g = clamp01((g - 0.5) * contrast + 0.5);
            b = clamp01((b - 0.5) * contrast + 0.5);

            // Highlights / shadows
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            if lum > 0.5 && hi != 0.0 {
                let t = (lum - 0.5) * 2.0;
                let adj = -hi * t * 0.35;
                r = clamp01(r + adj);
                g = clamp01(g + adj);
                b = clamp01(b + adj);
            } else if lum <= 0.5 && sh != 0.0 {
                let t = (0.5 - lum) * 2.0;
                let adj = sh * t * 0.35;
                r = clamp01(r + adj);
                g = clamp01(g + adj);
                b = clamp01(b + adj);
            }

            // Clarity (local contrast)
            if let Some(ref blur) = lum_blur {
                let idx = (y * w + x) as usize;
                let local = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let detail = local - blur[idx];
                let adj = detail * clarity * 1.5;
                r = clamp01(r + adj);
                g = clamp01(g + adj);
                b = clamp01(b + adj);
            }

            // Saturation
            let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            r = clamp01(gray + (r - gray) * sat);
            g = clamp01(gray + (g - gray) * sat);
            b = clamp01(gray + (b - gray) * sat);

            out.put_pixel(
                x,
                y,
                Rgb([
                    (r * 255.0).round().clamp(0.0, 255.0) as u8,
                    (g * 255.0).round().clamp(0.0, 255.0) as u8,
                    (b * 255.0).round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    out
}

fn box_blur_f32(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    if radius == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut tmp = vec![0f32; w * h];
    let mut dst = vec![0f32; w * h];
    // horizontal
    for y in 0..h {
        let row = y * w;
        let mut acc = 0f32;
        let mut count = 0usize;
        for x in 0..=radius.min(w - 1) {
            acc += src[row + x];
            count += 1;
        }
        for x in 0..w {
            tmp[row + x] = acc / count as f32;
            let leave = x as isize - radius as isize;
            let enter = x + radius + 1;
            if leave >= 0 {
                acc -= src[row + leave as usize];
                count -= 1;
            }
            if enter < w {
                acc += src[row + enter];
                count += 1;
            }
        }
    }
    // vertical
    for x in 0..w {
        let mut acc = 0f32;
        let mut count = 0usize;
        for y in 0..=radius.min(h - 1) {
            acc += tmp[y * w + x];
            count += 1;
        }
        for y in 0..h {
            dst[y * w + x] = acc / count as f32;
            let leave = y as isize - radius as isize;
            let enter = y + radius + 1;
            if leave >= 0 {
                acc -= tmp[leave as usize * w + x];
                count -= 1;
            }
            if enter < h {
                acc += tmp[enter * w + x];
                count += 1;
            }
        }
    }
    dst
}

pub fn export_jpeg(src: &Path, dest: &Path, params: &DevelopParams, quality: u8) -> Result<()> {
    let img = load_image(src)?;
    let developed = apply_develop(&img, params);
    let dyn_img = DynamicImage::ImageRgb8(developed);
    let q = quality.clamp(1, 100);
    let mut file = std::fs::File::create(dest)
        .with_context(|| format!("create export {}", dest.display()))?;
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, q);
    encoder
        .encode_image(&dyn_img)
        .context("encode JPEG")?;
    Ok(())
}

/// Encode developed preview RGB as egui ColorImage.
pub fn to_color_image(rgb: &RgbImage) -> egui::ColorImage {
    let (w, h) = rgb.dimensions();
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for p in rgb.pixels() {
        pixels.push(egui::Color32::from_rgb(p[0], p[1], p[2]));
    }
    egui::ColorImage {
        size: [w as usize, h as usize],
        pixels,
    }
}

pub fn load_thumb_color_image(path: &Path, max_edge: u32) -> Result<egui::ColorImage> {
    let img = load_image(path)?;
    // Nearest is much cheaper than Triangle for tiny library thumbs.
    let (w, h) = img.dimensions();
    let edge = w.max(h).max(1);
    let preview = if edge <= max_edge {
        img
    } else {
        let scale = max_edge as f32 / edge as f32;
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        img.resize(nw, nh, image::imageops::FilterType::Nearest)
    };
    let rgb = preview.to_rgb8();
    Ok(to_color_image(&rgb))
}

/// Encode a small JPEG thumb for on-disk cache (second launch stays fast).
pub fn write_thumb_jpeg(path: &Path, dest: &Path, max_edge: u32) -> Result<()> {
    let img = load_image(path)?;
    let (w, h) = img.dimensions();
    let edge = w.max(h).max(1);
    let preview = if edge <= max_edge {
        img
    } else {
        let scale = max_edge as f32 / edge as f32;
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        img.resize(nw, nh, image::imageops::FilterType::Nearest)
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let rgb = preview.to_rgb8();
    let mut file = std::fs::File::create(dest)?;
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 70);
    enc.encode_image(&DynamicImage::ImageRgb8(rgb))?;
    Ok(())
}
