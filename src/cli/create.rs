use std::{fs, path::PathBuf};

use gyat::root;

use crate::Result;

/// This create function takes in an Option<String> for name to handle both cases when name is given or not
pub fn create(name: &Option<String>) -> Result<()> {
    // Validate the repository name
    let repo_path = match name {
        Some(ref name) => {
            // Consider adding some more name validations
            if name.is_empty() || name == "." || name == ".." {
                return Err("Invalid repository name".into());
            }
            PathBuf::from(&name)
        }
        None => std::env::current_dir()?,
    };

    // Create the directory if a name was provided and it doesn't exist
    if name.is_some() && !repo_path.exists() {
        fs::create_dir(&repo_path)?;
    } else if repo_path.exists() && !repo_path.is_dir() {
        return Err(format!("{} exists but is not a directory", repo_path.display()).into());
    }

    // Create .gyat directory
    let gyat_path = repo_path.join(".gyat");
    if root::is_repo(&repo_path) {
        return Err("This is already inside a .gyat repository".into());
    }
    fs::create_dir(&gyat_path)?;

    // Create the internal structure
    let gyat_path_commits = gyat_path.join("commits");
    let gyat_path_dirs = gyat_path.join("dirs");
    let gyat_path_files = gyat_path.join("files");
    let gyat_path_head = gyat_path.join("HEAD");

    fs::create_dir(gyat_path_commits)?;
    fs::create_dir(gyat_path_dirs)?;
    fs::create_dir(gyat_path_files)?;
    fs::write(gyat_path.join("index"), "")?;
    fs::write(gyat_path_head, "")?;

    println!(
        "Initialized empty gyat repository in {}",
        repo_path.display()
    );
    Ok(())
}
