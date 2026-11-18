//! Folder scanning / importing. Strictly read-only on the original files.

use std::fs;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;
use walkdir::WalkDir;

use crate::db::{Db, DbError, NewPhoto, PhotoUpsert};
use crate::{edits, exif_meta, hashing, paths};

/// Extensions decoded directly by the `image` crate.
pub const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "jpe", "png", "webp", "tif", "tiff", "bmp", "gif"];

#[derive(Debug, Error)]
pub enum ScanError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Path(#[from] paths::PathError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub folder: String,
    pub scanned: u64,
    pub added: u64,
    pub skipped: u64,
    pub errors: Vec<String>,
}

pub fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            IMAGE_EXTS.contains(&e.as_str()) || edits::RAW_EXTS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

/// Stored `format` value: plain extension, or `raw:<ext>` for RAW files.
pub fn format_of(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if edits::RAW_EXTS.contains(&ext.as_str()) {
        format!("raw:{ext}")
    } else {
        ext
    }
}

/// Scan `folder` (recursively if `recursive`), register it as an imported folder,
/// and upsert every supported photo into the database.
pub fn scan_folder(db: &Db, folder: &Path, recursive: bool) -> Result<ImportResult, ScanError> {
    let canon = paths::canonical_dir(folder)?;
    let folder_str = canon.to_string_lossy().to_string();
    db.add_imported_folder(&folder_str, recursive)?;

    let mut res = ImportResult {
        folder: folder_str,
        scanned: 0,
        added: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    let max_depth = if recursive { usize::MAX } else { 1 };
    for entry in WalkDir::new(&canon)
        .follow_links(false)
        .max_depth(max_depth)
        .sort_by_file_name()
    {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                res.errors.push(format!("walk error: {e}"));
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_supported_file(path) {
            continue;
        }
        res.scanned += 1;
        match process_file(db, path) {
            Ok(PhotoUpsert::Added) => res.added += 1,
            Ok(_) => res.skipped += 1,
            Err(e) => res.errors.push(format!("{}: {e}", path.display())),
        }
    }
    Ok(res)
}

fn process_file(db: &Db, path: &Path) -> Result<PhotoUpsert, ScanError> {
    let path_str = path.to_string_lossy().to_string();
    let meta = fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        })
        .unwrap_or_default();
    let exif = exif_meta::read_exif(path);
    let (w, h) = edits::image_dimensions(path);
    let sha256 = hashing::sha256_file(path).ok();
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let np = NewPhoto {
        path: path_str,
        filename,
        size_bytes: meta.len() as i64,
        mtime,
        width: w,
        height: h,
        taken_at: exif.taken_at,
        camera: exif.camera,
        lens: exif.lens,
        iso: exif.iso,
        aperture: exif.aperture,
        focal_length: exif.focal_length,
        exposure_time: exif.exposure_time,
        gps_lat: exif.gps_lat,
        gps_lon: exif.gps_lon,
        format: format_of(path),
        sha256,
    };
    Ok(db.upsert_photo(&np)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_jpeg(dir: &Path, name: &str) {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");
        fs::copy(fixture, dir.join(name)).unwrap();
    }

    #[test]
    fn scans_folder_and_skips_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        fs::create_dir_all(photos.join("sub")).unwrap();
        write_jpeg(&photos, "a.jpg");
        write_jpeg(&photos.join("sub"), "b.jpg");
        write_jpeg(&photos, "c.cr2"); // raw ext, still scanned
        let mut txt = fs::File::create(photos.join("notes.txt")).unwrap();
        txt.write_all(b"not a photo").unwrap();

        let db = Db::open_in_memory().unwrap();
        let res = scan_folder(&db, &photos, true).unwrap();
        assert_eq!(res.scanned, 3);
        assert_eq!(res.added, 3);
        assert_eq!(res.skipped, 0);
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);

        // rescan: everything skipped (unchanged)
        let res2 = scan_folder(&db, &photos, true).unwrap();
        assert_eq!(res2.added, 0);
        assert_eq!(res2.skipped, 3);

        let list = db
            .list_photos(&crate::db::PhotoFilter {
                search: String::new(),
                sort: "filename_asc".to_string(),
                favorites_only: false,
                album_id: None,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(list.total, 3);
        let raw = list.photos.iter().find(|p| p.filename == "c.cr2").unwrap();
        assert_eq!(raw.format, "raw:cr2");
        assert_eq!(raw.width, Some(64));
        assert_eq!(raw.taken_at.as_deref(), Some("2024-05-01 10:20:30"));
        assert_eq!(raw.camera.as_deref(), Some("TestCam TestModel 2000"));
        assert!(raw.sha256.is_some());
        let jpg = list.photos.iter().find(|p| p.filename == "a.jpg").unwrap();
        assert_eq!(jpg.format, "jpg");

        // folders registered
        assert_eq!(db.list_imported_folders().unwrap().len(), 1);
        assert!(!db.folder_paths().unwrap().is_empty());
    }

    #[test]
    fn non_recursive_scan() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        fs::create_dir_all(photos.join("sub")).unwrap();
        write_jpeg(&photos, "top.jpg");
        write_jpeg(&photos.join("sub"), "nested.jpg");
        let db = Db::open_in_memory().unwrap();
        let res = scan_folder(&db, &photos, false).unwrap();
        assert_eq!(res.scanned, 1);
        assert_eq!(res.added, 1);
    }
}
