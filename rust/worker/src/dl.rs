//! HTTP download helper with SHA-256 verification and progress callbacks.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DlError {
    #[error("http error: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sha256 mismatch for {url}: expected {expected}, got {actual}")]
    Hash { url: String, expected: String, actual: String },
    #[error("bad zip: {0}")]
    Zip(String),
}

pub type ProgressFn<'a> = dyn FnMut(u64 /*done*/, u64 /*total*/) + 'a;

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(1800)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// Download `url` into memory (with optional progress callback).
pub fn download(url: &str, mut progress: Option<&mut ProgressFn>) -> Result<Vec<u8>, DlError> {
    let resp = agent()
        .get(url)
        .call()
        .map_err(|e| DlError::Http(format!("{url}: {e}")))?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let mut body = resp.into_body();
    let mut reader = body.as_reader();
    let mut out = Vec::new();
    let mut buf = [0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        done += n as u64;
        if let Some(p) = progress.as_deref_mut() {
            p(done, total);
        }
    }
    Ok(out)
}

/// Download `url` to `dest`, verifying SHA-256. Writes via tmp+rename.
pub fn download_to_file(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    mut progress: Option<&mut ProgressFn>,
) -> Result<(), DlError> {
    let bytes = download(url, progress.as_deref_mut())?;
    verify_sha256(&bytes, expected_sha256, url)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str, what: &str) -> Result<(), DlError> {
    let mut h = Sha256::new();
    h.update(bytes);
    let actual = hex::encode(h.finalize());
    if !actual.eq_ignore_ascii_case(expected_hex) {
        return Err(DlError::Hash {
            url: what.to_string(),
            expected: expected_hex.to_string(),
            actual,
        });
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String, DlError> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Extract a zip archive into `dest` (zip-slip safe).
pub fn unzip(bytes: &[u8], dest: &Path) -> Result<(), DlError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| DlError::Zip(e.to_string()))?;
    fs::create_dir_all(dest)?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| DlError::Zip(e.to_string()))?;
        let name = match f.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };
        let out = dest.join(name);
        if f.is_dir() {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut w = fs::File::create(&out)?;
            std::io::copy(&mut f, &mut w)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = f.unix_mode() {
                let _ = fs::set_permissions(&out, fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

/// Write `content` to `path` atomically (tmp + rename).
pub fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp-write");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Write a text file (used for JSON state files).
pub fn write_text(path: &Path, content: &str) -> std::io::Result<()> {
    write_atomic(path, content.as_bytes())
}

/// Read a text file, tolerating absence.
pub fn read_text_opt(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

pub fn stdout_write_line(line: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}
