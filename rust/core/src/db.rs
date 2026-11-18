//! SQLite library database: photos, imported folders, albums, edits, exports, settings.
//!
//! All structs serialize to the exact JSON shape the TypeScript frontend expects
//! (see `src/lib/types.ts`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Local;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

pub fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ---------------------------------------------------------------- types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub mtime: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub taken_at: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<String>,
    pub aperture: Option<String>,
    pub focal_length: Option<String>,
    pub exposure_time: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub format: String,
    pub sha256: Option<String>,
    pub favorite: bool,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhotoList {
    pub photos: Vec<Photo>,
    pub total: i64,
}

fn default_sort() -> String {
    "taken_desc".to_string()
}
fn default_limit() -> i64 {
    500
}

/// Query filter for [`Db::list_photos`]; mirrors `PhotoQuery` in TS.
#[derive(Debug, Clone, Deserialize)]
pub struct PhotoFilter {
    #[serde(default)]
    pub search: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default)]
    pub favorites_only: bool,
    #[serde(default)]
    pub album_id: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Album {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedFolder {
    pub id: i64,
    pub path: String,
    pub recursive: bool,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportRecord {
    pub id: i64,
    pub photo_id: i64,
    pub out_path: String,
    pub kind: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub sha256: String,
    pub photos: Vec<Photo>,
}

/// A freshly scanned photo, before it gets a database id.
#[derive(Debug, Clone)]
pub struct NewPhoto {
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub mtime: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub taken_at: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<String>,
    pub aperture: Option<String>,
    pub focal_length: Option<String>,
    pub exposure_time: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub format: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoUpsert {
    Added,
    Updated,
    Unchanged,
}

// ---------------------------------------------------------------- schema

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS photos (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  filename TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  mtime TEXT NOT NULL,
  width INTEGER,
  height INTEGER,
  taken_at TEXT,
  camera TEXT,
  lens TEXT,
  iso TEXT,
  aperture TEXT,
  focal_length TEXT,
  exposure_time TEXT,
  gps_lat REAL,
  gps_lon REAL,
  format TEXT NOT NULL,
  sha256 TEXT,
  favorite INTEGER NOT NULL DEFAULT 0,
  added_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_photos_sha256 ON photos(sha256);
CREATE INDEX IF NOT EXISTS idx_photos_taken ON photos(taken_at);

CREATE TABLE IF NOT EXISTS imported_folders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  recursive INTEGER NOT NULL DEFAULT 1,
  added_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS albums (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS album_photos (
  album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
  photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
  added_at TEXT NOT NULL,
  PRIMARY KEY (album_id, photo_id)
);

CREATE TABLE IF NOT EXISTS edits (
  photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
  params TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS exports (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
  out_path TEXT NOT NULL,
  kind TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

const PHOTO_COLS: &str = "id, path, filename, size_bytes, mtime, width, height, taken_at, camera, lens, \
  iso, aperture, focal_length, exposure_time, gps_lat, gps_lon, format, sha256, favorite, added_at";

fn row_to_photo(row: &rusqlite::Row) -> rusqlite::Result<Photo> {
    Ok(Photo {
        id: row.get(0)?,
        path: row.get(1)?,
        filename: row.get(2)?,
        size_bytes: row.get(3)?,
        mtime: row.get(4)?,
        width: row.get(5)?,
        height: row.get(6)?,
        taken_at: row.get(7)?,
        camera: row.get(8)?,
        lens: row.get(9)?,
        iso: row.get(10)?,
        aperture: row.get(11)?,
        focal_length: row.get(12)?,
        exposure_time: row.get(13)?,
        gps_lat: row.get(14)?,
        gps_lon: row.get(15)?,
        format: row.get(16)?,
        sha256: row.get(17)?,
        favorite: row.get::<_, i64>(18)? != 0,
        added_at: row.get(19)?,
    })
}

/// Escape a string for use in a SQL `LIKE ... ESCAPE '\'` pattern.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

// ---------------------------------------------------------------- Db

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (or create) a database file, creating parent directories as needed.
    pub fn open(path: &Path) -> Result<Db> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open_with_flags(path, OpenFlags::default())?;
        Self::init(conn)
    }

    /// Open a transient in-memory database (for tests).
    pub fn open_in_memory() -> Result<Db> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Db> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Db { conn })
    }

    /// Consistent online backup of the database to `dest`.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut dest_conn = Connection::open(dest)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest_conn)?;
        backup.run_to_completion(64, Duration::from_millis(50), None)?;
        Ok(())
    }

    // ------------------------------------------------------------ photos

    pub fn photo_exists(&self, path: &str) -> Result<bool> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM photos WHERE path = ?1", params![path], |r| r.get(0))?;
        Ok(n > 0)
    }

    /// Insert a new photo, or refresh metadata if the file changed on disk.
    pub fn upsert_photo(&self, p: &NewPhoto) -> Result<PhotoUpsert> {
        let existing: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT id, size_bytes, mtime FROM photos WHERE path = ?1",
                params![p.path],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        match existing {
            None => {
                self.conn.execute(
                    &format!(
                        "INSERT INTO photos ({PHOTO_COLS}) VALUES (NULL,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
                    ),
                    params![
                        p.path, p.filename, p.size_bytes, p.mtime, p.width, p.height, p.taken_at,
                        p.camera, p.lens, p.iso, p.aperture, p.focal_length, p.exposure_time,
                        p.gps_lat, p.gps_lon, p.format, p.sha256, 0, now_string()
                    ],
                )?;
                Ok(PhotoUpsert::Added)
            }
            Some((_id, size, mtime)) => {
                if size == p.size_bytes && mtime == p.mtime {
                    Ok(PhotoUpsert::Unchanged)
                } else {
                    self.conn.execute(
                        "UPDATE photos SET filename=?1, size_bytes=?2, mtime=?3, width=?4, height=?5, \
                         taken_at=?6, camera=?7, lens=?8, iso=?9, aperture=?10, focal_length=?11, \
                         exposure_time=?12, gps_lat=?13, gps_lon=?14, format=?15, sha256=?16 \
                         WHERE path=?17",
                        params![
                            p.filename, p.size_bytes, p.mtime, p.width, p.height, p.taken_at, p.camera,
                            p.lens, p.iso, p.aperture, p.focal_length, p.exposure_time, p.gps_lat,
                            p.gps_lon, p.format, p.sha256, p.path
                        ],
                    )?;
                    Ok(PhotoUpsert::Updated)
                }
            }
        }
    }

    pub fn get_photo(&self, id: i64) -> Result<Option<Photo>> {
        self.conn
            .query_row(
                &format!("SELECT {PHOTO_COLS} FROM photos WHERE id = ?1"),
                params![id],
                row_to_photo,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn photo_path(&self, id: i64) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT path FROM photos WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(DbError::from)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<()> {
        self.conn
            .execute("UPDATE photos SET favorite = ?1 WHERE id = ?2", params![favorite as i64, id])?;
        Ok(())
    }

    pub fn list_photos(&self, f: &PhotoFilter) -> Result<PhotoList> {
        let mut where_clauses: Vec<String> = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if !f.search.trim().is_empty() {
            where_clauses.push("p.filename LIKE ? ESCAPE '\\'".to_string());
            args.push(Box::new(format!("%{}%", escape_like(&f.search.trim()))));
        }
        if f.favorites_only {
            where_clauses.push("p.favorite = 1".to_string());
        }
        if let Some(aid) = f.album_id {
            where_clauses.push(
                "EXISTS (SELECT 1 FROM album_photos ap WHERE ap.photo_id = p.id AND ap.album_id = ?)"
                    .to_string(),
            );
            args.push(Box::new(aid));
        }
        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let order_sql = match f.sort.as_str() {
            "taken_asc" => "ORDER BY (p.taken_at IS NULL), p.taken_at ASC, p.id ASC",
            "filename_asc" => "ORDER BY p.filename COLLATE NOCASE ASC, p.id ASC",
            "filename_desc" => "ORDER BY p.filename COLLATE NOCASE DESC, p.id DESC",
            "size_desc" => "ORDER BY p.size_bytes DESC, p.id DESC",
            _ => "ORDER BY (p.taken_at IS NULL), p.taken_at DESC, p.id DESC", // taken_desc
        };

        let total: i64 = {
            let sql = format!("SELECT COUNT(*) FROM photos p {where_sql}");
            let refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|a| a.as_ref()).collect();
            self.conn.query_row(&sql, rusqlite::params_from_iter(refs), |r| r.get(0))?
        };

        let sql = format!(
            "SELECT {PHOTO_COLS} FROM photos p {where_sql} {order_sql} LIMIT ? OFFSET ?"
        );
        let mut all_args: Vec<&dyn rusqlite::types::ToSql> =
            args.iter().map(|a| a.as_ref()).collect();
        let limit = f.limit.clamp(1, 100_000);
        let offset = f.offset.max(0);
        all_args.push(&limit);
        all_args.push(&offset);
        let mut stmt = self.conn.prepare(&sql)?;
        let photos = stmt
            .query_map(rusqlite::params_from_iter(all_args), row_to_photo)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(PhotoList { photos, total })
    }

    /// Groups of photos that share the same SHA-256 (exact duplicates).
    pub fn find_duplicates(&self) -> Result<Vec<DuplicateGroup>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {PHOTO_COLS} FROM photos p \
             WHERE p.sha256 IS NOT NULL AND p.sha256 IN \
             (SELECT sha256 FROM photos WHERE sha256 IS NOT NULL GROUP BY sha256 HAVING COUNT(*) > 1) \
             ORDER BY p.sha256, p.id"
        ))?;
        let rows = stmt.query_map([], row_to_photo)?;
        let mut groups: Vec<DuplicateGroup> = Vec::new();
        for r in rows {
            let photo = r?;
            let sha = photo.sha256.clone().unwrap_or_default();
            if groups.last().map(|g| g.sha256.as_str()) != Some(sha.as_str()) {
                groups.push(DuplicateGroup { sha256: sha, photos: Vec::new() });
            }
            groups.last_mut().unwrap().photos.push(photo);
        }
        Ok(groups)
    }

    // ------------------------------------------------------------ folders

    pub fn add_imported_folder(&self, path: &str, recursive: bool) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO imported_folders (path, recursive, added_at) VALUES (?1, ?2, ?3)",
            params![path, recursive as i64, now_string()],
        )?;
        let id = self
            .conn
            .query_row("SELECT id FROM imported_folders WHERE path = ?1", params![path], |r| {
                r.get(0)
            })?;
        Ok(id)
    }

    pub fn list_imported_folders(&self) -> Result<Vec<ImportedFolder>> {
        let mut stmt =
            self.conn.prepare("SELECT id, path, recursive, added_at FROM imported_folders ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(ImportedFolder {
                id: r.get(0)?,
                path: r.get(1)?,
                recursive: r.get::<_, i64>(2)? != 0,
                added_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_imported_folder(&self, id: i64) -> Result<Option<ImportedFolder>> {
        self.conn
            .query_row(
                "SELECT id, path, recursive, added_at FROM imported_folders WHERE id = ?1",
                params![id],
                |r| {
                    Ok(ImportedFolder {
                        id: r.get(0)?,
                        path: r.get(1)?,
                        recursive: r.get::<_, i64>(2)? != 0,
                        added_at: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
    }

    /// Remove an imported folder and all photos that live under it.
    /// Returns the number of photo rows removed. Files on disk are untouched.
    pub fn remove_imported_folder(&self, id: i64) -> Result<u64> {
        let folder = match self.get_imported_folder(id)? {
            Some(f) => f,
            None => return Ok(0),
        };
        self.conn.execute("DELETE FROM imported_folders WHERE id = ?1", params![id])?;
        let esc = escape_like(&folder.path);
        let removed = self.conn.execute(
            "DELETE FROM photos WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\' OR path LIKE ?3 ESCAPE '\\'",
            params![folder.path, format!("{esc}/%"), format!("{esc}\\%")],
        )?;
        Ok(removed as u64)
    }

    /// Canonical paths of all imported folders (for path validation).
    pub fn folder_paths(&self) -> Result<Vec<PathBuf>> {
        let folders = self.list_imported_folders()?;
        Ok(folders
            .iter()
            .filter_map(|f| std::fs::canonicalize(&f.path).ok())
            .collect())
    }

    // ------------------------------------------------------------ edits

    pub fn get_edits(&self, photo_id: i64) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT params FROM edits WHERE photo_id = ?1", params![photo_id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(DbError::from)
    }

    pub fn save_edits(&self, photo_id: i64, params_json: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO edits (photo_id, params, updated_at) VALUES (?1, ?2, ?3)",
            params![photo_id, params_json, now_string()],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------ exports

    pub fn add_export(&self, photo_id: i64, out_path: &str, kind: &str) -> Result<ExportRecord> {
        self.conn.execute(
            "INSERT INTO exports (photo_id, out_path, kind, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![photo_id, out_path, kind, now_string()],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(ExportRecord {
            id,
            photo_id,
            out_path: out_path.to_string(),
            kind: kind.to_string(),
            created_at: now_string(),
        })
    }

    pub fn list_exports(&self, photo_id: i64) -> Result<Vec<ExportRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, photo_id, out_path, kind, created_at FROM exports WHERE photo_id = ?1 ORDER BY id DESC",
        )?;
        let rows = stmt.query_map(params![photo_id], |r| {
            Ok(ExportRecord {
                id: r.get(0)?,
                photo_id: r.get(1)?,
                out_path: r.get(2)?,
                kind: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    // ------------------------------------------------------------ albums

    pub fn list_albums(&self) -> Result<Vec<Album>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, a.created_at, \
             (SELECT COUNT(*) FROM album_photos ap WHERE ap.album_id = a.id) AS cnt \
             FROM albums a ORDER BY a.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Album {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                count: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn create_album(&self, name: &str) -> Result<Album> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DbError::Sql(rusqlite::Error::InvalidQuery));
        }
        self.conn.execute(
            "INSERT INTO albums (name, created_at) VALUES (?1, ?2)",
            params![name, now_string()],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Album { id, name: name.to_string(), created_at: now_string(), count: 0 })
    }

    pub fn delete_album(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM albums WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_to_album(&self, album_id: i64, photo_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO album_photos (album_id, photo_id, added_at) VALUES (?1, ?2, ?3)",
            params![album_id, photo_id, now_string()],
        )?;
        Ok(())
    }

    pub fn remove_from_album(&self, album_id: i64, photo_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM album_photos WHERE album_id = ?1 AND photo_id = ?2",
            params![album_id, photo_id],
        )?;
        Ok(())
    }

    pub fn album_contains(&self, album_id: i64, photo_id: i64) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM album_photos WHERE album_id = ?1 AND photo_id = ?2",
            params![album_id, photo_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    // ------------------------------------------------------------ settings

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
            .optional()
            .map_err(DbError::from)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }
}

// `.optional()` on query_row results
use rusqlite::OptionalExtension;

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_photo(path: &str, sha: &str) -> NewPhoto {
        NewPhoto {
            path: path.to_string(),
            filename: path.rsplit('/').next().unwrap().to_string(),
            size_bytes: 1234,
            mtime: "2024-05-01 10:00:00".to_string(),
            width: Some(64),
            height: Some(48),
            taken_at: Some("2024-05-01 10:20:30".to_string()),
            camera: Some("TestCam TestModel 2000".to_string()),
            lens: Some("TestLens 50mm".to_string()),
            iso: Some("400".to_string()),
            aperture: Some("f/2.8".to_string()),
            focal_length: Some("50 mm".to_string()),
            exposure_time: Some("1/125 s".to_string()),
            gps_lat: Some(48.625),
            gps_lon: Some(13.4),
            format: "jpg".to_string(),
            sha256: Some(sha.to_string()),
        }
    }

    #[test]
    fn upsert_and_list_photos() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.upsert_photo(&sample_photo("/p/a.jpg", "aaa")).unwrap(), PhotoUpsert::Added);
        assert_eq!(db.upsert_photo(&sample_photo("/p/a.jpg", "aaa")).unwrap(), PhotoUpsert::Unchanged);
        let mut changed = sample_photo("/p/a.jpg", "aaa");
        changed.size_bytes = 9999;
        assert_eq!(db.upsert_photo(&changed).unwrap(), PhotoUpsert::Updated);

        db.upsert_photo(&sample_photo("/p/b.png", "bbb")).unwrap();
        let mut c = sample_photo("/p/c.jpg", "ccc");
        c.taken_at = None;
        db.upsert_photo(&c).unwrap();

        let all = db
            .list_photos(&PhotoFilter {
                search: String::new(),
                sort: "taken_desc".to_string(),
                favorites_only: false,
                album_id: None,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(all.total, 3);
        assert_eq!(all.photos.len(), 3);
        // taken_desc: dated photos first (nulls last); equal dates tie-break by id DESC
        assert_eq!(all.photos[0].path, "/p/b.png");
        assert_eq!(all.photos[2].path, "/p/c.jpg");

        // search
        let r = db
            .list_photos(&PhotoFilter {
                search: "b.pn".to_string(), // '_' is a LIKE wildcard escaped by us
                sort: "filename_asc".to_string(),
                favorites_only: false,
                album_id: None,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.photos[0].filename, "b.png");

        // favorites
        db.set_favorite(1, true).unwrap();
        let favs = db
            .list_photos(&PhotoFilter {
                search: String::new(),
                sort: "filename_asc".to_string(),
                favorites_only: true,
                album_id: None,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(favs.total, 1);
        assert!(favs.photos[0].favorite);
    }

    #[test]
    fn duplicates_grouping() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_photo(&sample_photo("/p/1.jpg", "same")).unwrap();
        db.upsert_photo(&sample_photo("/p/2.jpg", "same")).unwrap();
        db.upsert_photo(&sample_photo("/p/3.jpg", "unique")).unwrap();
        let dups = db.find_duplicates().unwrap();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].sha256, "same");
        assert_eq!(dups[0].photos.len(), 2);
    }

    #[test]
    fn folders_and_albums_and_settings() {
        let db = Db::open_in_memory().unwrap();
        let fid = db.add_imported_folder("/photos", true).unwrap();
        assert_eq!(db.add_imported_folder("/photos", false).unwrap(), fid); // idempotent
        assert_eq!(db.list_imported_folders().unwrap().len(), 1);

        db.upsert_photo(&sample_photo("/photos/a.jpg", "x")).unwrap();
        db.upsert_photo(&sample_photo("/elsewhere/b.jpg", "y")).unwrap();
        let removed = db.remove_imported_folder(fid).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(db.list_imported_folders().unwrap().len(), 0);
        assert_eq!(db.list_photos(&PhotoFilter {
            search: String::new(),
            sort: "filename_asc".to_string(),
            favorites_only: false,
            album_id: None,
            limit: 100,
            offset: 0,
        }).unwrap().total, 1);

        let a = db.create_album("Trip").unwrap();
        db.upsert_photo(&sample_photo("/e/c.jpg", "z")).unwrap();
        db.add_to_album(a.id, 2).unwrap();
        assert!(db.album_contains(a.id, 2).unwrap());
        let albums = db.list_albums().unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].count, 1);
        let filtered = db.list_photos(&PhotoFilter {
            search: String::new(),
            sort: "filename_asc".to_string(),
            favorites_only: false,
            album_id: Some(a.id),
            limit: 100,
            offset: 0,
        }).unwrap();
        assert_eq!(filtered.total, 1);
        db.remove_from_album(a.id, 2).unwrap();
        assert!(!db.album_contains(a.id, 2).unwrap());
        db.delete_album(a.id).unwrap();
        assert!(db.list_albums().unwrap().is_empty());

        assert_eq!(db.get_setting("theme").unwrap(), None);
        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap().as_deref(), Some("dark"));

        db.save_edits(2, r#"{"rotate_deg":90}"#).unwrap();
        assert_eq!(db.get_edits(2).unwrap().as_deref(), Some(r#"{"rotate_deg":90}"#));
        assert_eq!(db.get_edits(999).unwrap(), None);

        let rec = db.add_export(2, "/out/x.jpg", "edit").unwrap();
        assert_eq!(rec.photo_id, 2);
        assert_eq!(db.list_exports(2).unwrap().len(), 1);
    }

    #[test]
    fn backup_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("lib.db");
        let db = Db::open(&db_path).unwrap();
        db.upsert_photo(&sample_photo("/p/a.jpg", "aaa")).unwrap();
        let bak = dir.path().join("backup.db");
        db.backup_to(&bak).unwrap();
        let db2 = Db::open(&bak).unwrap();
        let p = db2.get_photo(1).unwrap().expect("photo in backup");
        assert_eq!(p.path, "/p/a.jpg");
        assert_eq!(p.camera.as_deref(), Some("TestCam TestModel 2000"));
    }
}
