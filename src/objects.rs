#![allow(dead_code)]
use crate::{
    hash,
    utils::{gyat_paths, AllPaths},
    Result,
};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
};

use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};

/// Gets the compressed format of a blob as a vector of bytes.
/// For this implementation, only the contents of `blob`s are compressed.
///
/// Note: before calling this function, make sure that there's no `blob` with the same SHA1 already
/// stored in the repository.
///
/// * `blob_file`: the file to generate a blob for. Must be a file.
/// # Return values
/// - Err for any I/O error encountered.
/// - Ok(Vec<u8>) where the vector is the compressed content if nothing goes wrong.
pub fn format_blob_content(blob_source: &mut File) -> Result<Vec<u8>> {
    debug_assert!(blob_source.metadata()?.is_file());

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let mut reader = BufReader::new(blob_source);
    let mut buf: [u8; 1024] = [0; 1024];
    while {
        buf.fill(0);
        reader.read(&mut buf[..])? > 0
    } {
        encoder.write_all(&buf)?;
    }

    Ok(encoder.finish()?)
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
/// Either a blob (file/symlink?) or a tree (directory).
pub enum FType {
    Blob,
    Tree,
}

#[derive(Debug, PartialEq, Eq)]
/// One of a commit, a File(FType::Blob) or a File(FTYpe::Tree).
pub enum ObjType {
    Commit,
    File(FType),
}

#[derive(Debug, Hash, PartialEq, Eq)]
/// Includes tree and blob objects.
///
/// * `ftype`:
/// * `hash`:
/// * `component`:
pub struct FileObject {
    pub ftype: FType,
    pub hash: [u8; 20],
    pub component: OsString,
}

/// Commit object only.
///
/// Note: if we ever reach branching, we may increase the number of parents to 2.
///
/// * `parent`:
/// * `root`:
/// * `datetime`: currently unused
pub struct CommitObject {
    pub parent: Option<[u8; 20]>,
    pub root: [u8; 20],
    // pub datetime: DateTime<Local>,
}

impl FileObject {
    /// Gets a `FileObjectRef`.
    pub fn as_ref(&self) -> FileObjectRef<'_> {
        FileObjectRef {
            ftype: self.ftype,
            hash: &self.hash,
            component: &self.component,
        }
    }

    /// Gets a `FileObjectRef` from mutable.
    pub fn as_mut_ref(&mut self) -> FileObjectRef<'_> {
        FileObjectRef {
            ftype: self.ftype,
            hash: &self.hash,
            component: &self.component,
        }
    }
}

#[derive(Debug)]
/// Like `FileObject`, just without the ownership.
///
/// * `ftype`:
/// * `hash`:
/// * `component`:
pub struct FileObjectRef<'a> {
    pub ftype: FType,
    pub hash: &'a [u8; 20],
    pub component: &'a OsStr,
}

impl PartialEq for dyn FObj {
    fn eq(&self, other: &Self) -> bool {
        self.ftype() == other.ftype()
            && self.hash() == other.hash()
            && self.component() == other.component()
    }
}

// DO NOT IMPLEMENT MORE OF THIS TRAIT THAN THE ONES ABOVE.
pub trait FObj {
    fn ftype(&self) -> FType;
    fn hash(&self) -> &[u8; 20];
    fn component(&self) -> &OsStr;
}

impl FObj for FileObject {
    #[inline]
    fn ftype(&self) -> FType {
        self.ftype
    }

    #[inline]
    fn hash(&self) -> &[u8; 20] {
        &self.hash
    }

    #[inline]
    fn component(&self) -> &OsStr {
        &self.component
    }
}

impl<'a> FObj for FileObjectRef<'a> {
    #[inline]
    fn ftype(&self) -> FType {
        self.ftype
    }

    #[inline]
    fn hash(&self) -> &[u8; 20] {
        self.hash
    }

    #[inline]
    fn component(&self) -> &OsStr {
        self.component
    }
}

/// Gets the content of the tree object.
/// The caller needs to figure out all information about the children of this tree.
/// After this, one can pass this function's return value to hash::create_sha1_hash and get the
/// SHA1.
///
/// * `children`: An iterator through a tuple containing the following:
/// - The type of the child.
/// - The SHA1 hash of the child.
/// - The component of that child as an OsStr.
///   - So, for example, you are building a tree object for `src`, and the child this tuple
///     represents is the directory `src/cli`, then the component expected is `cli`.
///   - If you want to add child `src/cli/mod.rs` for example, you need to build the tree object
///     for `src/cli` then use that to build the tree for `src`.
///
/// # Note
/// - This function does no I/O, so it doesn't return any error, unlike its counterpart
///   `get_blob_content`. It only formats the data passed into it.
pub fn format_tree_content<'a>(children: impl Iterator<Item = FileObjectRef<'a>>) -> Vec<u8> {
    let mut ret = Vec::new();
    for c in children {
        let type_str = match c.ftype {
            FType::Blob => "blob",
            FType::Tree => "tree",
        };
        let child_hash = hash::to_string(c.hash);
        ret.extend(type_str.as_bytes());
        ret.push(b'\t');
        ret.extend(child_hash.as_bytes());
        ret.push(b'\t');
        ret.extend(c.component.as_encoded_bytes());
        ret.push(b'\n');
    }

    ret
}

/// Reads the contents of the specified tree object.
///
/// # Return value
/// - Err if:
///   - I/O errors (eg, interrupted, file not exist), or,
///   - Convert-to-SHA1 error.
/// - Ok(Vec<FileObject>) otherwise. The contents of the vector is the file objects recorded inside
///   the tree node.
///
/// * `tree_hash`:
pub fn read_tree_content(tree_hash: &[u8; 20]) -> Result<Vec<FileObject>> {
    let AllPaths { dirs_path, .. } = gyat_paths()?;
    let tree_path = dirs_path.join(hash::to_string(tree_hash));
    if !tree_path.exists() {
        return Err(format!("Tree hash {} doesn't exist", hash::to_string(tree_hash)).into());
    }

    let mut ret = Vec::new();
    // so, it will probably throw when not enough permissions somehow.
    let mut reader = BufReader::new(File::open(&tree_path)?);
    let mut buf = String::new();
    while {
        buf.clear();
        reader.read_line(&mut buf)? > 0
    } {
        let parts = buf.trim().split('\t').collect::<Vec<_>>();
        let ftype = match parts[0].trim() {
            "blob" => FType::Blob,
            "tree" => FType::Tree,
            _ => {
                return Err(format!("Invalid file type format in {}", &tree_path.display()).into());
            }
        };
        let hash = hash::from_string(parts[1])?;
        let component = parts[2];
        ret.push(FileObject {
            ftype,
            hash,
            component: component.into(),
        });
    }

    Ok(ret)
}

///
/// # Return values:
/// - Err if I/O error.
/// - Ok(HashMap) otherwise.
///   - The key of the HashMap is the path relative to the directory represented by `root_hash`.
///   - The value of the HashMap is the corresponding SHA1 to that path.
///
/// * `root_hash`: It's called `root_hash` due to the relative path.
pub fn get_blobs_from_root(root_hash: &[u8; 20]) -> Result<HashMap<PathBuf, [u8; 20]>> {
    let mut ret = HashMap::new();
    let mut stack: Vec<(FType, PathBuf, [u8; 20])> = Vec::new();
    stack.extend(
        read_tree_content(root_hash)?
            .into_iter()
            .map(|fo| (fo.ftype, PathBuf::from(fo.component), fo.hash)),
    );

    while let Some(obj) = stack.pop() {
        use FType::*;
        match obj.0 {
            Blob => {
                ret.insert(obj.1, obj.2);
            }
            Tree => stack.extend(
                read_tree_content(&obj.2)?
                    .into_iter()
                    .map(|fo| (fo.ftype, obj.1.join(fo.component), fo.hash)),
            ),
        }
    }

    Ok(ret)
}

/// For now this ignores the list of changes, since I don't need it right now. But I will add it
/// later.
///
/// # Return values
/// - Err if I/O error.
/// - Ok(CommitObject) otherwise.
///
/// * `commit_hash`:
pub fn read_commit_content(commit_hash: &[u8; 20]) -> Result<CommitObject> {
    let AllPaths { commits_path, .. } = gyat_paths()?;
    let commit_file = commits_path.join(hash::to_string(commit_hash));
    if !commit_file.exists() {
        return Err(format!("Commit hash {} not exist", hash::to_string(commit_hash)).into());
    }

    let mut reader = BufReader::new(File::open(commit_file)?);
    let mut buf = String::new();
    if reader.read_line(&mut buf)? == 0 {
        return Err(format!("Commit file {} empty", commits_path.display()).into());
    }
    // the first should always be parent.
    let parts = buf.split(':').collect::<Vec<_>>();
    let parent = if parts[1].trim().len() < 20 {
        None
    } else {
        Some(hash::from_string(parts[1].trim())?)
    };
    buf.clear();

    if reader.read_line(&mut buf)? == 0 {
        return Err(format!("Commit file {} empty", commits_path.display()).into());
    }
    // this one should be Tree.
    let parts = buf.split(':').collect::<Vec<_>>();
    let root = hash::from_string(parts[1].trim()).unwrap();

    Ok(CommitObject { parent, root })
}

/// Reading file content from a blob
pub fn read_blob(blob_hash: &[u8; 20]) -> Result<Vec<u8>> {
    // Get the files_path
    let AllPaths { files_path, .. } = gyat_paths()?;
    let blob_path = files_path.join(hash::to_string(blob_hash));
    if !blob_path.exists() {
        return Err(format!("Blob hash {} doesn't exist", hash::to_string(blob_hash)).into());
    }

    let file = File::open(blob_path)?;

    // Using ZlibDecoder to decode the file content
    let mut decoder = ZlibDecoder::new(file);
    let mut content = Vec::new();
    decoder.read_to_end(&mut content)?;
    let last_nonzero = content
        .iter()
        .rposition(|b| *b != 0)
        .unwrap_or(content.len());
    Ok(content.into_iter().take(last_nonzero + 1).collect())
}
