//! Non-destructive edit pipeline (crop / rotate / flip / exposure / contrast /
//! brightness / saturation / sharpen) and atomic JPEG export.
//!
//! Originals are never modified: rendering always reads the source file and
//! produces new JPEG bytes.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, GenericImageView, RgbImage};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::exif_meta;

#[derive(Debug, Error)]
pub enum EditError {
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot decode file: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, EditError>;

/// Camera RAW extensions. These are previewed through their embedded JPEG.
pub const RAW_EXTS: &[&str] = &[
    "cr2", "cr3", "nef", "arw", "dng", "orf", "rw2", "pef", "raf", "srw", "x3f",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CropRect {
    /// relative 0..1
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditParams {
    #[serde(default)]
    pub crop: Option<CropRect>,
    /// degrees clockwise (frontend uses multiples of 90)
    #[serde(default)]
    pub rotate_deg: f64,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    /// stops, -3..3
    #[serde(default)]
    pub exposure: f64,
    /// -100..100
    #[serde(default)]
    pub contrast: f64,
    /// -100..100
    #[serde(default)]
    pub saturation: f64,
    /// -100..100
    #[serde(default)]
    pub brightness: f64,
    /// 0..100
    #[serde(default)]
    pub sharpen: f64,
}

impl Default for EditParams {
    fn default() -> Self {
        EditParams {
            crop: None,
            rotate_deg: 0.0,
            flip_h: false,
            flip_v: false,
            exposure: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            brightness: 0.0,
            sharpen: 0.0,
        }
    }
}

pub fn is_raw_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Load any supported file as a [`DynamicImage`]. RAW files are decoded from
/// their embedded EXIF preview JPEG.
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    if is_raw_file(path) {
        if let Some(jpg) = exif_meta::read_thumbnail(path) {
            return Ok(image::load_from_memory(&jpg)?);
        }
        return Err(EditError::Unsupported(format!(
            "no embedded preview in {}",
            path.display()
        )));
    }
    Ok(image::open(path)?)
}

/// Pixel dimensions of the stored image (full-size dims for RAW via EXIF).
pub fn image_dimensions(path: &Path) -> (Option<i64>, Option<i64>) {
    if is_raw_file(path) {
        if let Some((w, h)) = exif_meta::pixel_dimensions(path) {
            return (Some(w), Some(h));
        }
        if let Some(jpg) = exif_meta::read_thumbnail(path) {
            if let Ok(img) = image::load_from_memory(&jpg) {
                return (Some(img.width() as i64), Some(img.height() as i64));
            }
        }
        return (None, None);
    }
    match image::image_dimensions(path) {
        Ok((w, h)) => (Some(w as i64), Some(h as i64)),
        Err(_) => (None, None),
    }
}

/// Apply the full edit pipeline. Pure function of (image, params).
pub fn apply_params(img: DynamicImage, p: &EditParams) -> DynamicImage {
    let mut img = img;

    // 1. crop (relative rect -> pixels)
    if let Some(c) = &p.crop {
        let (w, h) = img.dimensions();
        if w > 0 && h > 0 {
            let cw = ((c.w * w as f64).round() as u32).clamp(1, w);
            let ch = ((c.h * h as f64).round() as u32).clamp(1, h);
            let x = ((c.x * w as f64).round() as u32).min(w - cw);
            let y = ((c.y * h as f64).round() as u32).min(h - ch);
            img = img.crop_imm(x, y, cw, ch);
        }
    }

    // 2. rotate (exact quarter turns; other angles snap to nearest 90)
    let deg = (p.rotate_deg.round() as i64).rem_euclid(360);
    let snapped = ((deg + 45) / 90 * 90).rem_euclid(360);
    img = match snapped {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    };

    // 3. flips
    if p.flip_h {
        image::imageops::flip_horizontal_in_place(&mut img);
    }
    if p.flip_v {
        image::imageops::flip_vertical_in_place(&mut img);
    }

    // 4. tonal / color adjustments on RGB8
    let mut rgb = img.to_rgb8();
    adjust_rgb(&mut rgb, p);
    DynamicImage::ImageRgb8(rgb)
}

fn adjust_rgb(img: &mut RgbImage, p: &EditParams) {
    let exposure_gain = 2f32.powf(p.exposure.clamp(-3.0, 3.0) as f32);
    let c = (p.contrast.clamp(-100.0, 100.0) / 100.0) as f32 * 255.0;
    let contrast_factor = (259.0 * (c + 255.0)) / (255.0 * (259.0 - c));
    let brightness_add = (p.brightness.clamp(-100.0, 100.0) / 100.0) as f32 * 255.0;
    let sat = (1.0 + p.saturation.clamp(-100.0, 100.0) / 100.0) as f32;

    let n = img.as_raw().len() / 3;
    for i in 0..n {
        let px = &mut img.as_mut()[3 * i..3 * i + 3];
        let mut r = px[0] as f32;
        let mut g = px[1] as f32;
        let mut b = px[2] as f32;
        // exposure
        r *= exposure_gain;
        g *= exposure_gain;
        b *= exposure_gain;
        // contrast around midpoint
        r = contrast_factor * (r - 128.0) + 128.0;
        g = contrast_factor * (g - 128.0) + 128.0;
        b = contrast_factor * (b - 128.0) + 128.0;
        // brightness
        r += brightness_add;
        g += brightness_add;
        b += brightness_add;
        // saturation (lerp towards luma)
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        r = luma + (r - luma) * sat;
        g = luma + (g - luma) * sat;
        b = luma + (b - luma) * sat;
        px[0] = r.clamp(0.0, 255.0) as u8;
        px[1] = g.clamp(0.0, 255.0) as u8;
        px[2] = b.clamp(0.0, 255.0) as u8;
    }

    // 5. sharpen (unsharp mask against a gaussian blur)
    let amount = (p.sharpen.clamp(0.0, 100.0) / 100.0) as f32;
    if amount > 0.0 {
        let src = img.clone();
        let blurred = image::imageops::blur(&src, 1.2);
        let s = src.as_raw();
        let bl = blurred.as_raw();
        let out = img.as_mut();
        for i in 0..out.len() {
            let v = s[i] as f32 + amount * 1.5 * (s[i] as f32 - bl[i] as f32);
            out[i] = v.clamp(0.0, 255.0) as u8;
        }
    }
}

/// Encode an image as JPEG bytes.
pub fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100));
    img.write_with_encoder(enc)?;
    Ok(buf.into_inner())
}

/// Render the edited image to JPEG bytes, downscaled so the longest side is at
/// most `max_dim` (0 = full resolution).
pub fn render(src: &Path, p: &EditParams, max_dim: u32) -> Result<Vec<u8>> {
    let img = load_image(src)?;
    let mut out = apply_params(img, p);
    if max_dim > 0 {
        let (w, h) = out.dimensions();
        let m = w.max(h);
        if m > max_dim {
            let scale = max_dim as f64 / m as f64;
            let nw = ((w as f64 * scale).round() as u32).max(1);
            let nh = ((h as f64 * scale).round() as u32).max(1);
            out = DynamicImage::ImageRgb8(image::imageops::resize(
                &out.to_rgb8(),
                nw,
                nh,
                image::imageops::FilterType::Lanczos3,
            ));
        }
    }
    encode_jpeg(&out, 92)
}

/// Export the edited image at full resolution to `out` (atomic write via tmp+rename).
/// Returns the output dimensions.
pub fn export_file(src: &Path, p: &EditParams, out: &Path) -> Result<(u32, u32)> {
    let img = load_image(src)?;
    let out_img = apply_params(img, p);
    let (w, h) = out_img.dimensions();
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let bytes = encode_jpeg(&out_img, 95)?;
    let tmp = out.with_extension("ps-tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, out)?;
    Ok((w, h))
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;
    use std::io::Write;

    fn fixture_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg")
    }

    fn make_test_image(dir: &tempfile::TempDir, w: u32, h: u32, name: &str) -> std::path::PathBuf {
        let mut img = RgbImage::new(w, h);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128u8]);
        }
        let p = dir.path().join(name);
        let mut f = fs::File::create(&p).unwrap();
        let mut buf = Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90)
            .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        f.write_all(&buf.into_inner()).unwrap();
        p
    }

    #[test]
    fn render_identity_matches_dimensions() {
        let p = fixture_path();
        let bytes = render(&p, &EditParams::default(), 0).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (64, 48));
    }

    #[test]
    fn render_downscales_to_max_dim() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_test_image(&dir, 800, 400, "big.jpg");
        let bytes = render(&p, &EditParams::default(), 200).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (200, 100));
    }

    #[test]
    fn crop_and_rotate() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_test_image(&dir, 200, 100, "r.jpg");
        let params = EditParams {
            crop: Some(CropRect { x: 0.25, y: 0.0, w: 0.5, h: 1.0 }),
            rotate_deg: 90.0,
            ..Default::default()
        };
        let bytes = render(&p, &params, 0).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        // crop 100x100 -> rotate 90 -> 100x100
        assert_eq!((img.width(), img.height()), (100, 100));
    }

    #[test]
    fn exposure_brightens() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_test_image(&dir, 20, 20, "e.jpg");
        let dark = render(&p, &EditParams::default(), 0).unwrap();
        let bright = render(
            &p,
            &EditParams { exposure: 1.0, ..Default::default() },
            0,
        )
        .unwrap();
        let d = image::load_from_memory(&dark).unwrap().to_rgb8();
        let b = image::load_from_memory(&bright).unwrap().to_rgb8();
        let avg = |img: &RgbImage| -> f32 {
            img.as_raw().iter().map(|&v| v as f32).sum::<f32>() / img.as_raw().len() as f32
        };
        assert!(avg(&b) > avg(&d) + 20.0, "+1ev should be clearly brighter");
    }

    #[test]
    fn export_file_is_atomic_and_writes_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let src = make_test_image(&dir, 50, 30, "src.jpg");
        let out = dir.path().join("nested/out.jpg");
        let params = EditParams { flip_h: true, rotate_deg: 180.0, ..Default::default() };
        let (w, h) = export_file(&src, &params, &out).unwrap();
        assert_eq!((w, h), (50, 30));
        let bytes = fs::read(&out).unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
        // original untouched
        let orig = fs::read(&src).unwrap();
        assert_eq!(&orig[0..2], &[0xFF, 0xD8]);
        assert!(!dir.path().join("nested/out.ps-tmp").exists());
    }

    #[test]
    fn raw_file_uses_embedded_preview() {
        // copy the jpeg fixture under a .cr2 name: TIFF-based raws are parsed by
        // kamadak-exif; here the JPEG content still exposes the thumbnail IFD.
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("shot.cr2");
        fs::copy(fixture_path(), &raw).unwrap();
        assert!(is_raw_file(&raw));
        let img = load_image(&raw).unwrap();
        assert!(img.width() > 0);
        let (w, h) = image_dimensions(&raw);
        assert_eq!((w, h), (Some(64), Some(48)));
    }
}
