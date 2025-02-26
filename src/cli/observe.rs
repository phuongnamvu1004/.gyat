use crate::Result;
use gyat::fs::ChangeType;
use gyat::{fs, utils};
use gyat::{hash, objects};
use std::collections::HashMap;
use std::env::current_dir;
use std::io::{BufRead, BufReader};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// `observe` for a list of paths.
///
/// * `paths`: list of `PathBuf`s.
pub fn observe(paths: &[PathBuf]) -> Result<()> {
    debug_assert!(!paths.is_empty());
    let utils::AllPaths {
        repo_root,
        gyat_path,
        index_path,
        ..
    } = utils::gyat_paths()?;

    let repo_root_relative = current_dir()?.strip_prefix(&repo_root)?.to_owned();
    // build the regex

    let matcher = {
        let mut regex_string = String::from("^.gyat");
        if let Ok(f) = File::open(Path::join(&repo_root, ".gyatignore")) {
            let mut reader = BufReader::new(f);
            let mut buf = String::new();
            while {
                buf.clear();
                reader.read_line(&mut buf)? > 0
            } {
                std::fmt::write(&mut regex_string, format_args!("|{}", buf.trim()))?;
            }
        };
        rare::RARE::new(&regex_string)?
    };

    let mut index_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(index_path)?;

    let mut observe_list: Vec<ObservedContent> = Vec::new();
    for path in paths.iter() {
        // this guarantees that for this dirtree, any leaf inside the tree is a file.
        for subdir in fs::get_files_and_syms(path)? {
            let root_relative = fs::normalize(
                &[&repo_root, &repo_root_relative, &subdir]
                    .iter()
                    .collect::<PathBuf>(),
            );
            if !matcher.is_match(&root_relative.strip_prefix(&repo_root)?.to_string_lossy()) {
                observe_list.push(observe_single_path(&root_relative, &repo_root).unwrap());
            }
        }
    }

    // check modification status.
    // We only care about files that are changed.
    if let Some(prev_root) = fs::get_root_tree_hash(&gyat_path, None)? {
        // these blobs were in both the last commit tree and the staged tree.
        let mut prev_comp: HashMap<PathBuf, [u8; 20]> =
            objects::get_blobs_from_root(&hash::from_string(&prev_root).unwrap())?
                .into_iter()
                .filter(|pair| {
                    for p in paths {
                        if pair
                            .0
                            .starts_with(fs::normalize(&repo_root_relative.join(p)))
                        {
                            return true;
                        }
                    }
                    false
                })
                .collect();
        // technically I don't need to return here but I want the nice message.
        // if prev_comp.is_empty() {
        //     println!("No change observed");
        //     return Ok(());
        // }
        //
        write_changes(&mut index_file, &observe_list, &mut prev_comp)?;
    } else {
        // there's no previous commit yet.
        for oc in observe_list {
            write_blob_index(
                &mut index_file,
                ObservedContentRef {
                    perm: oc.perm,
                    hash: &oc.hash,
                    path: &oc.path,
                    change: ChangeType::New,
                },
            )?;
        }
    }

    Ok(())
}

/// Write changes with ChangeType::New or ChangeType::Mod. Just a helper function for `observe`.
/// This function is only called when there are changes compared to the last commit observed (so,
/// there needs to be a previous commit and between them there are changes observed).
///
/// * `index_file`: the file to write to. `.gyat/index`
/// * `observe_list`:
/// * `prev_comp`:
fn write_changes(
    index_file: &mut File,
    observe_list: &[ObservedContent],
    prev_comp: &mut HashMap<PathBuf, [u8; 20]>,
) -> Result<()> {
    // the logic: for each file:
    // - if it doesn't exist in the last commit tree, it is a new file.
    // - if its SHA1 does change, it is modified.
    // - if its SHA1 doesn't change, it is unchanged and we don't need to track it.
    //
    // finally, anything that is in the last commit tree but not in the current commit tree in
    // `prev_comp` was deleted.
    for ObservedContent { hash, path, perm } in observe_list {
        if !prev_comp.contains_key(path) {
            write_blob_index(
                index_file,
                ObservedContentRef {
                    perm: *perm,
                    hash,
                    path,
                    change: ChangeType::New,
                },
            )?;
            continue;
        }
        // it contains the key now.
        let prev_hash = prev_comp.get(path).unwrap();
        if hash != prev_hash {
            write_blob_index(
                index_file,
                ObservedContentRef {
                    perm: *perm,
                    hash,
                    path,
                    change: ChangeType::Mod,
                },
            )?;
        }
        prev_comp.remove(path);
    }
    for del_blob in prev_comp {
        write_blob_index(
            index_file,
            ObservedContentRef {
                // lazy ass me.
                perm: b'1',
                hash: del_blob.1,
                path: del_blob.0,
                change: ChangeType::Del,
            },
        )?;
    }
    Ok(())
}

/// The thing passed into `write_blob_index`
///
/// * `perm`: Whether the file is readonly (in which case, this is 0) or not (1).
/// * `hash`: A pointer to the SHA1 array.
/// * `path`: The path of the source file `observe`d.
struct ObservedContentRef<'a> {
    perm: u8,
    hash: &'a [u8; 20],
    path: &'a Path,
    change: ChangeType,
}

struct ObservedContent {
    perm: u8,
    hash: [u8; 20],
    path: PathBuf,
}

/// `observe` for a single path.
///
/// # Return values
/// - Err if there's I/O error.
///
/// * `path`: the path. Make sure the path is a file.
/// * `repo_root`: `path` must be in `repo_root`.
/// * `index_file`: the ".gyat/index" file.
fn observe_single_path(path: &Path, repo_root: &Path) -> Result<ObservedContent> {
    if !path.exists() {
        return Err(format!("{} doesn't exist", path.display()).into());
    }
    if !path.starts_with(repo_root) {
        return Err(format!(
            "Path {} is not in repository root {}",
            path.display(),
            repo_root.display()
        )
        .into());
    }

    let mut blob_source = File::open(path)?;
    let perm = path.metadata()?.permissions();
    let hash = hash::digest_file(&mut blob_source)?;
    Ok(ObservedContent {
        perm: if perm.readonly() { b'0' } else { b'1' },
        hash,
        path: path.strip_prefix(repo_root)?.to_owned(),
    })
}

/// Writes the contents specified in `contents` as a single line into the `index_file`.
///
/// * `index_file`: .gyat/index.
/// * `contents`: struct `ObservedContent`.
fn write_blob_index(index_file: &mut File, contents: ObservedContentRef) -> Result<()> {
    let mut write_buf: Vec<u8> = Vec::new();

    write_buf.push(contents.perm);
    write_buf.push(b'\t');
    // literally a "linear map" from u8 to u8.
    write_buf.extend(hash::to_string(contents.hash).as_bytes());
    write_buf.push(b'\t');
    write_buf.extend(contents.path.as_os_str().as_encoded_bytes());
    write_buf.push(b'\t');
    write_buf.extend(format!("{:?}", contents.change).as_bytes());
    write_buf.push(b'\n');
    index_file.write_all(&write_buf)?;
    write_buf.clear();

    Ok(())
}
