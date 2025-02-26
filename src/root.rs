use std::path::{Path, PathBuf};

/// Whether there's a `.gyat` directory in `path` or its parent(s).
///
/// * `path`: the path to check.
pub fn is_repo(path: &Path) -> bool {
    get_repo_root(path).is_some()
}

/// # Returns
/// - Some(PathBuf) if this path or one of its parents is a `gyat` repository, with value as
///   the path to the repository that has `.gyat` in it.
/// - None otherwise.
///
/// * `path`: the path to check
pub fn get_repo_root(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let mut path = path.canonicalize().unwrap_or_default();
    // TOCTOU gonna scare the shit out of us, until we realize it's not relevant to our
    // project.
    // I (Huy) will need to look up to see if there's a cross-platform file-locking crate.
    let mut exists = path.join(".gyat").exists();
    while !exists {
        match path.parent() {
            None => return None,
            Some(p) => path = p.to_path_buf(),
        }
        exists = path.join(".gyat").exists();
    }
    Some(path)
}
