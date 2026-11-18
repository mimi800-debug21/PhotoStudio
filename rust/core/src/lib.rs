//! Core library of PhotoStudio.
//!
//! Everything here is platform independent and unit-testable without a GUI:
//! - [`db`]: SQLite library database (photos, albums, edits, exports, folders, settings)
//! - [`scan`]: folder scanning / importing (read-only on the original files)
//! - [`exif_meta`]: EXIF / GPS extraction
//! - [`edits`]: non-destructive edit pipeline (crop/rotate/exposure/.../sharpen) and atomic export
//! - [`thumb`]: thumbnail cache
//! - [`hashing`]: SHA-256 of files (duplicate detection groundwork)
//! - [`paths`]: path validation helpers

pub mod db;
pub mod edits;
pub mod exif_meta;
pub mod hashing;
pub mod paths;
pub mod scan;
pub mod thumb;

pub use db::{Album, Db, DuplicateGroup, ExportRecord, ImportedFolder, Photo, PhotoFilter, PhotoList};
pub use edits::{CropRect, EditParams};
