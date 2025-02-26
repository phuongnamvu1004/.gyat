use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Component, Path, PathBuf},
};

use crate::Result;
use crate::{dirtree::Tree, hash};

/// No I/O normalization.
///
/// * `path`: 
pub fn normalize(path: &Path) -> PathBuf {
    let mut ret = PathBuf::new();
    for comp in path.components() {
        use Component::*;
        match comp {
            Prefix(pf) => ret.push(pf.as_os_str()),
            RootDir => ret.push("/"),
            CurDir => {}
            ParentDir => {
                ret.pop();
            }
            Normal(n) => ret.push(n),
        }
    }

    ret
}

/// Traverses the given path.
///
/// # Parameters
/// * `path`: the given path
///
/// # Returns
/// - A Vec of PathBufs
pub fn traverse_path(path: &Path) -> Result<Vec<PathBuf>> {
    let mut ret = Vec::new();
    let mut pathbuf_queue: VecDeque<PathBuf> = VecDeque::new();
    pathbuf_queue.push_back(path.to_path_buf());
    // technically BFS, but this is a tree. So no need for a HashSet here.
    // Another way of doing this is using recursion.

    while let Some(pathbuf) = pathbuf_queue.pop_front() {
        if !pathbuf.is_dir() {
            ret.push(pathbuf);
            continue;
        }

        // I think the only possible error here is "lack of permission"
        for p in pathbuf.read_dir()? {
            // same here. If it's an error then just ignore that directory.
            let p = match p {
                Ok(p) => p,
                Err(_) => continue,
            };
            pathbuf_queue.push_back(p.path());
        }
        ret.push(pathbuf);
    }

    Ok(ret)
}

#[inline]
pub fn get_files_and_dirs(path: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    Ok(traverse_path(path)?.into_iter().partition(|p| p.is_dir()))
}

#[inline]
pub fn get_files_and_syms(path: &Path) -> Result<Vec<PathBuf>> {
    Ok(traverse_path(path)?
        .into_iter()
        .filter(|p| p.is_file() || p.is_symlink())
        .collect())
}

#[inline]
pub fn get_dirs(path: &Path) -> Result<Vec<PathBuf>> {
    Ok(traverse_path(path)?
        .into_iter()
        .filter(|p| p.is_dir())
        .collect())
}

/// An entry read by `read_index`
///
/// * `perm`:
/// * `hash`:
/// * `path`:
pub struct IndexEntry {
    pub perm: u8,
    pub hash: [u8; 20],
    pub path: PathBuf,
    pub change: ChangeType,
}

/// Reads the (new-format) index file.
///
/// * `index_file`:
pub fn read_index(index_file: &mut File) -> Result<Vec<IndexEntry>> {
    let mut files = Vec::new();
    let mut reader = BufReader::new(index_file);
    let mut buf = String::new();
    while {
        buf.clear();
        reader.read_line(&mut buf)? > 0
    } {
        let parts: Vec<_> = buf.trim().split('\t').collect();
        let perm = parts[0].parse::<u8>().unwrap();
        let hash = hash::from_string(parts[1])?;
        let path = PathBuf::from(parts[2]);
        let change = match parts[3] {
            "New" => ChangeType::New,
            "Mod" => ChangeType::Mod,
            "Del" => ChangeType::Del,
            _ => return Err(format!("Invalid change {}", parts[3]).into()),
        };

        files.push(IndexEntry {
            perm,
            hash,
            path,
            change,
        })
    }

    Ok(files)
}

#[derive(Debug)]
pub enum ChangeType {
    New,
    Mod,
    Del,
}

pub fn see_changes(
    observed_files: Vec<(u8, String, PathBuf)>,
    blob_map: &mut HashMap<PathBuf, String>,
    dirtree: &mut Tree,
) -> Result<Vec<(ChangeType, PathBuf)>> {
    let mut changes = Vec::new();

    for (_, idx_hash, path) in observed_files {
        match blob_map.remove(&path) {
            Some(blob_hash) => {
                if blob_hash == idx_hash {
                    //Unchanged
                    continue;
                } else {
                    //Modified
                    dirtree.add_path(&path);
                    changes.push((ChangeType::Mod, path));
                }
            }
            None => {
                //New
                dirtree.add_path(&path);
                changes.push((ChangeType::New, path));
            }
        }
    }

    for (deleted_path, _) in blob_map.drain() {
        //Deleted
        changes.push((ChangeType::Del, deleted_path));
    }

    Ok(changes)
}

/// [Nam Vu] I modified this method so that it can get any root tree hash from a specified commit, and if None is given it will just return the lastest commit
pub fn get_root_tree_hash(gyat_path: &Path, commit_hash: Option<&String>) -> Result<Option<String>> {
    // If no commit hash is provided, default to HEAD
    let commit_hash = match commit_hash {
        Some(hash) => hash.to_string(),
        None => fs::read_to_string(gyat_path.join("HEAD"))?.trim().to_string(),
    };

    if commit_hash.is_empty() {
        return Ok(None);
    }

    let commit_path = gyat_path.join("commits").join(commit_hash);
    let commit_content = fs::read_to_string(commit_path)?;

    let root_tree_hash = commit_content
        .lines()
        .find(|line| line.starts_with("Tree: "))
        .map(|hash| hash[6..].to_string())
        .ok_or("Missing tree")?;

    Ok(Some(root_tree_hash))
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, fs, io::Read};

    use super::*;

    #[ignore = "This tests the default behavior of File and ReadDir. Run with --show-output"]
    #[test]
    fn rando() {
        // run this with --show-output
        {
            let mut f = fs::File::open("src").unwrap();
            let mut data = vec![];
            // src is a directory
            assert!(f.read_to_end(&mut data).is_err());
        }
        for dir in fs::read_dir("src").unwrap() {
            let dir = dir.unwrap();
            println!("{}", dir.file_name().into_string().unwrap());
        }
    }

    #[test]
    /// Checks the traversal function.
    fn test_traversal() {
        let ret_pathbufs = traverse_path(Path::new("test-data")).unwrap();
        let path_hash: HashSet<PathBuf> = vec![
            Path::new("test-data").into(),
            Path::new("test-data/another-test-dir").into(),
            Path::new("test-data/cargo-mimic.txt").into(),
        ]
        .into_iter()
        .collect();

        for pb in &ret_pathbufs {
            assert!(
                path_hash.contains(pb),
                "Update path_hash inside this test according to test-data contents before retrying!"
            );
        }

        let ret_pathbufs = get_dirs(Path::new("test-data")).unwrap();
        let path_hash: HashSet<PathBuf> = vec![
            Path::new("test-data").into(),
            Path::new("test-data/another-test-dir").into(),
        ]
        .into_iter()
        .collect();

        for pb in &ret_pathbufs {
            assert!(
                path_hash.contains(pb),
                "Update path_hash inside this test according to test-data contents before retrying!"
            );
        }

        let ret_pathbufs = get_files_and_syms(Path::new("test-data")).unwrap();
        let path_hash: HashSet<PathBuf> = vec![Path::new("test-data/cargo-mimic.txt").into()]
            .into_iter()
            .collect();

        for pb in &ret_pathbufs {
            assert!(
                path_hash.contains(pb),
                "Update path_hash inside this test according to test-data contents before retrying!"
            );
        }
    }
}
