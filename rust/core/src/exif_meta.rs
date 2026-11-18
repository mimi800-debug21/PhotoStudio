//! EXIF / GPS metadata extraction (read-only) using kamadak-exif.

use kamadak_exif as exif;
use serde::Serialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Read EXIF metadata from any file kamadak-exif understands (JPEG, TIFF-based
/// RAW, PNG, WebP, ...). Returns `None` on any hard parse failure; non-fatal
/// errors are tolerated (partial results kept) for robustness with quirky files.
fn read_exif_container(path: &Path) -> Option<exif::Exif> {
    let f = File::open(path).ok()?;
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    match reader.read_from_container(&mut BufReader::new(f)) {
        Ok(e) => Some(e),
        Err(e) => e.distill_partial_result(|_| {}).ok(),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExifInfo {
    pub taken_at: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<String>,
    pub aperture: Option<String>,
    pub focal_length: Option<String>,
    pub exposure_time: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

fn ascii(value: &exif::Value) -> Option<String> {
    if let exif::Value::Ascii(parts) = value {
        let s = parts
            .iter()
            .map(|p| String::from_utf8_lossy(p).to_string())
            .collect::<String>();
        let s = s.trim().trim_matches('\0').to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

fn rational_f64(value: &exif::Value, idx: usize) -> Option<f64> {
    if let exif::Value::Rational(rs) = value {
        rs.get(idx).map(|r| r.to_f64())
    } else {
        None
    }
}

fn short_u32(value: &exif::Value, idx: usize) -> Option<u32> {
    match value {
        exif::Value::Short(v) => v.get(idx).map(|&x| x as u32),
        exif::Value::Long(v) => v.get(idx).copied(),
        _ => None,
    }
}

/// Normalize "YYYY:MM:DD HH:MM:SS" to "YYYY-MM-DD HH:MM:SS".
pub fn normalize_datetime(s: &str) -> Option<String> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let date = &s[0..10];
    let time = &s[11..19];
    if date.chars().nth(4) == Some(':') && date.chars().nth(7) == Some(':') {
        Some(format!("{}-{}-{} {}", &date[0..4], &date[5..7], &date[8..10], time))
    } else {
        Some(format!("{} {}", date, time))
    }
}

fn format_exposure_time(value: &exif::Value) -> Option<String> {
    // ExposureTime is a single RATIONAL (num/den packed in one Rational)
    let v = match value {
        exif::Value::Rational(rs) => rs.first().map(|r| r.to_f64())?,
        _ => return None,
    };
    if v <= 0.0 {
        return None;
    }
    if v >= 1.0 {
        Some(format!("{:.1} s", v))
    } else {
        Some(format!("1/{:.0} s", (1.0 / v).round()))
    }
}

fn format_aperture(value: &exif::Value) -> Option<String> {
    let v = rational_f64(value, 0)?;
    if v <= 0.0 {
        return None;
    }
    Some(format!("f/{:.1}", v))
}

fn format_focal(value: &exif::Value) -> Option<String> {
    let v = rational_f64(value, 0)?;
    if v <= 0.0 {
        return None;
    }
    Some(format!("{:.0} mm", v))
}

fn gps_coord(value: &exif::Value, ref_ascii: Option<&String>) -> Option<f64> {
    let d = rational_f64(value, 0)?;
    let m = rational_f64(value, 1).unwrap_or(0.0);
    let s = rational_f64(value, 2).unwrap_or(0.0);
    let mut v = d + m / 60.0 + s / 3600.0;
    if let Some(r) = ref_ascii {
        if r == "S" || r == "W" {
            v = -v;
        }
    }
    Some(v)
}

/// Read EXIF from a file. Never fails hard: returns default (empty) info on any error.
pub fn read_exif(path: &Path) -> ExifInfo {
    let mut info = ExifInfo::default();
    let exif = match read_exif_container(path) {
        Some(e) => e,
        None => return info,
    };
    let mut make: Option<String> = None;
    let mut model: Option<String> = None;
    let mut lat_ref: Option<String> = None;
    let mut lon_ref: Option<String> = None;
    for field in exif.fields() {
        match field.tag {
            exif::Tag::DateTimeOriginal => {
                if info.taken_at.is_none() {
                    if let Some(s) = ascii(&field.value) {
                        info.taken_at = normalize_datetime(&s);
                    }
                }
            }
            exif::Tag::DateTime => {
                if info.taken_at.is_none() {
                    if let Some(s) = ascii(&field.value) {
                        info.taken_at = normalize_datetime(&s);
                    }
                }
            }
            exif::Tag::Make => make = ascii(&field.value).or(make),
            exif::Tag::Model => model = ascii(&field.value).or(model),
            exif::Tag::LensModel => info.lens = ascii(&field.value),
            exif::Tag::PhotographicSensitivity | exif::Tag::ISOSpeed => {
                if info.iso.is_none() {
                    info.iso = short_u32(&field.value, 0).map(|v| v.to_string());
                }
            }
            exif::Tag::FNumber => {
                if info.aperture.is_none() {
                    info.aperture = format_aperture(&field.value);
                }
            }
            exif::Tag::FocalLength => {
                if info.focal_length.is_none() {
                    info.focal_length = format_focal(&field.value);
                }
            }
            exif::Tag::ExposureTime => {
                if info.exposure_time.is_none() {
                    info.exposure_time = format_exposure_time(&field.value);
                }
            }
            exif::Tag::GPSLatitudeRef => lat_ref = ascii(&field.value),
            exif::Tag::GPSLatitude => {
                info.gps_lat = gps_coord(&field.value, lat_ref.as_ref());
            }
            exif::Tag::GPSLongitudeRef => lon_ref = ascii(&field.value),
            exif::Tag::GPSLongitude => {
                info.gps_lon = gps_coord(&field.value, lon_ref.as_ref());
            }
            _ => {}
        }
    }
    let cam = match (make, model) {
        (Some(m), Some(mo)) => Some(format!("{} {}", m, mo).trim().to_string()),
        (Some(m), None) => Some(m),
        (None, Some(mo)) => Some(mo),
        (None, None) => None,
    };
    info.camera = cam.filter(|s| !s.is_empty());
    info
}

fn field_u32(exif: &exif::Exif, tag: exif::Tag) -> Option<u32> {
    for field in exif.fields() {
        if field.tag == tag {
            return match &field.value {
                exif::Value::Short(v) => v.first().map(|&x| x as u32),
                exif::Value::Long(v) => v.first().copied(),
                _ => None,
            };
        }
    }
    None
}

/// Best-effort pixel dimensions of the *full-size* image from EXIF tags.
pub fn pixel_dimensions(path: &Path) -> Option<(i64, i64)> {
    let exif = read_exif_container(path)?;
    let w = field_u32(&exif, exif::Tag::PixelXDimension).or_else(|| field_u32(&exif, exif::Tag::ImageWidth));
    let h = field_u32(&exif, exif::Tag::PixelYDimension).or_else(|| field_u32(&exif, exif::Tag::ImageLength));
    match (w, h) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w as i64, h as i64)),
        _ => None,
    }
}

/// Extract the embedded JPEG preview/thumbnail stored in the EXIF thumbnail IFD.
/// Works for JPEG files and TIFF-based RAW files (CR2, NEF, ARW, DNG, ...).
pub fn read_thumbnail(path: &Path) -> Option<Vec<u8>> {
    let exif = read_exif_container(path)?;
    let off = exif.get_field(exif::Tag::JPEGInterchangeFormat, exif::In::THUMBNAIL)?;
    let len = exif.get_field(exif::Tag::JPEGInterchangeFormatLength, exif::In::THUMBNAIL)?;
    let to_usize = |v: &exif::Value| -> Option<usize> {
        match v {
            exif::Value::Long(v) => v.first().map(|&x| x as usize),
            exif::Value::Short(v) => v.first().map(|&x| x as usize),
            _ => None,
        }
    };
    let o = to_usize(&off.value)?;
    let l = to_usize(&len.value)?;
    if l == 0 {
        return None;
    }
    let buf = exif.buf();
    if o + l <= buf.len() {
        let slice = &buf[o..o + l];
        // sanity: must look like a JPEG (FFD8 ... FFD9)
        if slice.len() >= 4 && slice[0] == 0xFF && slice[1] == 0xD8 {
            return Some(slice.to_vec());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_datetime_works() {
        assert_eq!(
            normalize_datetime("2024:05:01 10:20:30"),
            Some("2024-05-01 10:20:30".to_string())
        );
        assert_eq!(normalize_datetime("bad"), None);
    }

    #[test]
    fn reads_fixture_exif() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");
        let info = read_exif(&path);
        assert_eq!(info.taken_at.as_deref(), Some("2024-05-01 10:20:30"));
        assert_eq!(info.camera.as_deref(), Some("TestCam TestModel 2000"));
        assert_eq!(info.lens.as_deref(), Some("TestLens 50mm"));
        assert_eq!(info.iso.as_deref(), Some("400"));
        assert_eq!(info.aperture.as_deref(), Some("f/2.8"));
        assert_eq!(info.focal_length.as_deref(), Some("50 mm"));
        assert_eq!(info.exposure_time.as_deref(), Some("1/125 s"));
        let lat = info.gps_lat.expect("gps lat");
        let lon = info.gps_lon.expect("gps lon");
        assert!((lat - (48.0 + 37.0 / 60.0 + 30.0 / 3600.0)).abs() < 1e-9);
        assert!((lon - (13.0 + 24.0 / 60.0)).abs() < 1e-9);
    }

    #[test]
    fn reads_embedded_thumbnail_and_dims() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");
        let thumb = read_thumbnail(&path).expect("fixture should carry an embedded JPEG thumbnail");
        assert!(thumb.len() > 100);
        assert_eq!(&thumb[0..2], &[0xFF, 0xD8]);
        // decodable as JPEG
        let img = image::load_from_memory(&thumb).expect("thumbnail must decode");
        assert!(img.width() > 0 && img.height() > 0);
        let dims = pixel_dimensions(&path);
        assert!(dims.is_some(), "pixel dimensions should be readable from EXIF");
    }
}
