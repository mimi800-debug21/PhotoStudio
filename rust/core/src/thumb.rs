//! On-disk thumbnail cache: `<app_data>/thumbs/<photo_id>_<size>.jpg`.

use std::fs;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use thiserror::Error;

use crate::edits::{self, EditError};

#[derive(Debug, Error)]
pub enum ThumbError {
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ThumbError>;

pub struct ThumbCache {
    pub dir: PathBuf,
}

impl ThumbCache {
    pub fn new(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn file(&self, photo_id: i64, size: u32) -> PathBuf {
        self.dir.join(format!("{photo_id}_{size}.jpg"))
    }

    /// Return cached JPEG thumbnail bytes, generating and caching them on miss.
    pub fn get_or_make(&self, photo_id: i64, src: &Path, size: u32) -> Result<Vec<u8>> {
        let size = size.clamp(16, 4096);
        let f = self.file(photo_id, size);
        if let Ok(bytes) = fs::read(&f) {
            if bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
                return Ok(bytes);
            }
        }
        let img = edits::load_image(src)?;
        let rgb = img.to_rgb8();
        let thumb = image::imageops::thumbnail(&rgb, size, size);
        let bytes = edits::encode_jpeg(&DynamicImage::ImageRgb8(thumb), 80)?;
        let tmp = self.dir.join(format!("{photo_id}_{size}.tmp"));
        fs::write(&tmp, &bytes)?;
        let _ = fs::rename(&tmp, &f); // best effort: keep serving fresh bytes on failure
        Ok(bytes)
    }

    /// Delete all cached sizes for a photo.
    pub fn invalidate(&self, photo_id: i64) {
        if let Ok(rd) = fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with(&format!("{photo_id}_")) {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }

    /// Remove the whole cache.
    pub fn clear(&self) -> Result<()> {
        if let Ok(rd) = fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let _ = fs::remove_file(e.path());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_and_caches_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ThumbCache::new(dir.path().join("thumbs")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");

        let b1 = cache.get_or_make(7, &fixture, 64).unwrap();
        assert_eq!(&b1[0..2], &[0xFF, 0xD8]);
        let img = image::load_from_memory(&b1).unwrap();
        assert!(img.width() <= 64 && img.height() <= 64);
        assert!(img.width() > 0);

        // second call served from cache file
        assert!(cache.file(7, 64).exists());
        let b2 = cache.get_or_make(7, &fixture, 64).unwrap();
        assert_eq!(b1, b2);

        cache.invalidate(7);
        assert!(!cache.file(7, 64).exists());
    }
}
