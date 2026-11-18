use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("path does not exist: {0}")]
    NotFound(String),
    #[error("path is not a directory: {0}")]
    NotDir(String),
    #[error("path is not allowed (outside imported folders / app data): {0}")]
    NotAllowed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Canonicalize a path (resolves symlinks / short names). Fails if it does not exist.
pub fn canonical(p: &Path) -> Result<PathBuf, PathError> {
    let c = p.canonicalize()?;
    Ok(c)
}

/// Ensure `p` is a directory and return its canonical path.
pub fn canonical_dir(p: &Path) -> Result<PathBuf, PathError> {
    let c = canonical(p)?;
    if !c.is_dir() {
        return Err(PathError::NotDir(c.display().to_string()));
    }
    Ok(c)
}

/// Check that `p` (canonicalized) is inside one of `allowed_roots`.
pub fn is_under_any(p: &Path, allowed_roots: &[PathBuf]) -> bool {
    allowed_roots.iter().any(|root| p.starts_with(root))
}

/// Validate a photo file path: it must exist and be inside one of the imported
/// folders or inside the app data directory (for exports / temp files).
pub fn validate_photo_path(
    p: &Path,
    imported_folders: &[PathBuf],
    app_data: &Path,
) -> Result<PathBuf, PathError> {
    let c = canonical(p)?;
    if !c.is_file() {
        return Err(PathError::NotFound(c.display().to_string()));
    }
    let mut roots = imported_folders.to_vec();
    roots.push(app_data.to_path_buf());
    if is_under_any(&c, &roots) {
        Ok(c)
    } else {
        Err(PathError::NotAllowed(c.display().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_any_works() {
        let root = PathBuf::from("/home/user/photos");
        assert!(is_under_any(Path::new("/home/user/photos/a.jpg"), &[root.clone()]));
        assert!(is_under_any(Path::new("/home/user/photos/sub/a.jpg"), &[root.clone()]));
        assert!(!is_under_any(Path::new("/home/user/photos2/a.jpg"), &[root.clone()]));
        assert!(!is_under_any(Path::new("/etc/passwd"), &[root]));
    }
}
