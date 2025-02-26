//! Additional utilities that I don't know where to put.

use crate::root;

use crate::Result;
use std::path::PathBuf;

/// All the useful paths we may need.
/// Not too performant, but too nice to pass.
///
/// * `repo_root`: the directory with the `.gyat` directory inside.
/// * `gyat_path`: `repo_root.join(".gyat")`.
/// * `index_path`: `gyat_path.join("index")`.
/// * `commits_path`: `gyat_path.join("commits")`.
/// * `trees_path`:
/// * `files_path`:
pub struct AllPaths {
    pub repo_root: PathBuf,
    pub gyat_path: PathBuf,
    pub index_path: PathBuf,
    pub head_path: PathBuf,
    pub commits_path: PathBuf,
    pub dirs_path: PathBuf,
    pub files_path: PathBuf,
}
/// Convenient function to get all the paths we may need.
/// This assumes a `gyat` repository already exists, and hence cannot be used
/// inside the function `create::create`.
/// Not too performant, but too nice to pass.
///
/// Then, one can unpack it to get the paths one needs.
///
/// # Returns
/// - Err if `current_dir()` is not in a gyat repository.
/// - Ok with a struct containing all the paths otherwise.
pub fn gyat_paths() -> Result<AllPaths> {
    let repo_root = root::get_repo_root(std::env::current_dir()?.as_path())
        .ok_or("Current directory in not in gyat repository")?;
    let gyat_path = repo_root.join(".gyat");
    let index_path = gyat_path.join("index");
    let head_path = gyat_path.join("HEAD");
    let commits_path = gyat_path.join("commits");
    let dirs_path = gyat_path.join("dirs");
    let files_path = gyat_path.join("files");
    Ok(AllPaths {
        repo_root,
        gyat_path,
        index_path,
        head_path,
        commits_path,
        dirs_path,
        files_path,
    })
}
