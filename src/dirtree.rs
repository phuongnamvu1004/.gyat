use crate::{
    hash, objects,
    utils::{gyat_paths, AllPaths},
    Result,
};

// There are some nice quirks I will abuse to make my life easier:
// 1. If I removed a node, I would want to remove all its children anyways.
// 2. The root of the tree is always the repository root.
// 3. You can only add new nodes to either the root or a non-leaf nodes.
// 4. The main program only cares about leaf nodes.
//
// What does this try to model?
// 1. Say, if I run `observe -p src/cli.rs src/hash.rs src`, before 'src' is added to the tree,
//    there are already `src/cli.rs` and `src/hash.rs` as `src`'s children. If I don't remove the
//    children, `src` isn't a leaf node anymore. I now care about all files/directories inside `src`,
//    so I don't want `src` to NOT be a leaf node. Look at it, that command is technically
//    equivalent to `observe -p src`.
// 2. I mean, why not.
// 3. When the tree is first created, the root node is the leaf node. But I still want to add stuff
//    in. And why only add to non-leaves: from point 1, the directory I'm trying to add into
//    already covered that node.
// 4. Basically the logic of point 1.
//
// UPDATE:
//
// As of writing this update, everything leaf in `dirtree` is expected to represent file/blob.

use std::{
    cmp::Reverse, collections::{BinaryHeap, HashMap}, ffi::{OsStr, OsString}, fs::{self, File}, io::{Seek, SeekFrom}, path::{Component, Path, PathBuf}
};

use crate::root;

// not very cache-line-efficient since it's a big chongus, but anyways.
// You never expect any tree data structure to be cache efficient in the first place.

#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents a node of the directory tree.
///
/// # Members
/// * `children`
/// * `filename`: name of the file/directory this node represents.
///   Should have been named `component` to be honest, 'cuz that's actually what it is.
/// * `parent`: the parent of this node.
///
/// # Notes
/// * Any function taking in a `usize` as parent/child only makes sense in the context of the `Tree`
///   this node is in.
pub struct TreeNode {
    children: HashMap<OsString, usize>,
    // file or directory. Invalid nodes (aka, nodes removed from the tree) will have empty
    // filename.
    filename: OsString,
    parent: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Tree {
    repo_root: PathBuf,
    nodes: Vec<TreeNode>,
    // I'm planning to use size = 0 as a signal that the only node being concerned is the
    // repository root. Due to alignment, adding a bool (1 byte) makes this struct 8 bytes heavier.
    size: usize,
    // IIRC this is max-heap by default. Reverse is a wrapper that, well, reverses that, so it's a
    // min-heap. Why min-heap? Consider the nodes vector kinda like an allocator. If we always use
    // the first memory slot available, it's a lot more efficient.
    next_frees: BinaryHeap<Reverse<usize>>,
}

pub enum ObjectType {
    Blob,
    Tree,
}

impl TreeNode {
    #[inline]
    pub fn get_file_name(&self) -> &OsStr {
        &self.filename
    }

    #[inline]
    fn new(filename: &OsStr) -> Self {
        Self {
            children: HashMap::new(),
            filename: filename.to_owned(),
            parent: None,
        }
    }

    #[inline]
    /// Add a parent.
    /// The only node not having a parent is the root node.
    ///
    /// * `parent`: The index of the parent of this `TreeNode` inside the `Tree` this node is in.
    fn add_parent(&mut self, parent: usize) {
        self.parent = Some(parent);
    }

    #[inline]
    /// # Return value
    /// * If `false`, this node has a child with the same component as `path_comp` already.
    /// * Otherwise, the child is added successfully.
    ///
    /// # Parameters
    /// * `path_comp`: The component directly under this `TreeNode`.
    /// * `child`: The index of the child inside the `Tree` this `TreeNode` is in.
    fn add_child(&mut self, path_comp: &OsStr, child: usize) -> bool {
        if self.children.contains_key(path_comp) {
            return false;
        }
        self.children.insert(path_comp.to_owned(), child);
        true
    }

    #[inline]
    /// Remove all children of this `TreeNode`.
    ///
    /// # Return value
    /// - The number of children before removing.
    fn remove_children(&mut self) -> usize {
        let ret = self.children.len();
        self.children.clear();
        ret
    }

    #[inline]
    /// # Return value
    /// - None if there's no child directly under this `TreeNode` containing the filename.
    ///
    /// # Parameters
    /// * `component`
    ///
    /// # Notes
    /// - This used to be called `get_child`, but changed into `get_component` to reflect its
    ///   actual use case.
    /// - It was called `get_child` because of convention (a tree node has pointers to its
    ///   "children").
    fn get_component(&self, component: &OsStr) -> Option<usize> {
        self.children.get(component).copied()
    }

    #[inline]
    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    #[inline]
    fn is_valid(&self) -> bool {
        !self.filename.is_empty()
    }
}

impl Tree {
    pub fn new() -> Result<Self> {
        Ok(Self {
            repo_root: root::get_repo_root(Path::new("."))
                .ok_or("The current working directory is not in any repository")?,
            nodes: vec![TreeNode::new(Path::new(".").as_os_str())],
            size: 1,
            next_frees: BinaryHeap::new(),
        })
    }

    pub fn leaves(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.nodes
            .iter()
            .filter(|n| n.is_leaf() && n.is_valid())
            .map(|n| self.relative_path(n))
    }

    /// Traverses down the tree starting from the root to see if the path in question exists.
    ///
    /// * `path`:
    pub fn contains_path(&self, path: &Path) -> bool {
        if let Some(p) = root::get_repo_root(path) {
            if p != self.repo_root {
                return false;
            }
        }
        if self.only_repo_root() {
            return false;
        }

        let path = if !path.is_absolute() {
            path
        } else {
            path.strip_prefix(root::get_repo_root(path).unwrap())
                .unwrap()
        };

        let mut idx = 0;
        for comp in path
            .components()
            // Basically takes out any '.', since if that thing is at the beginning of the path,
            // this loop is fucked.
            // Why it's fucked if so: it check if there's a child of '.' (repo root) named '.',
            // which is an automatic no.
            //
            // Update, now that may be obsolete since whatever I pass in `add_path`, I canonicalize
            // it.
            .filter(|cp| !matches!(cp, Component::CurDir))
            .map(|c| c.as_os_str())
        {
            if self.nodes[idx].is_leaf() {
                return true;
            }
            match self.nodes[idx].get_component(comp) {
                None => return false,
                Some(i) => idx = i,
            }
        }
        true
    }

    /// Cannot remove the repository root.
    ///
    /// * `path`:
    pub fn remove_path(&self, path: &Path) -> bool {
        if let Some(p) = root::get_repo_root(path) {
            if p != self.repo_root {
                return false;
            }
        }
        if self.only_repo_root() {
            return false;
        }

        // TODO: maybe I will allow removal of elements from the dirtree.
        #[allow(unused_variables)]
        let path = if !path.is_absolute() {
            path
        } else {
            path.strip_prefix(root::get_repo_root(path).unwrap())
                .unwrap()
        };

        false
    }

    pub fn add_path(&mut self, path: &Path) -> bool {
        if !path.exists() {
            return false;
        }
        if let Some(r) = root::get_repo_root(path) {
            if r != self.repo_root {
                return false;
            }
        } else {
            return false;
        }
        // if the repo root is/was added, anything else is ignored.
        if self.only_repo_root() {
            return false;
        }
        if path
            .canonicalize()
            .unwrap()
            .strip_prefix(&self.repo_root)
            .unwrap()
            .as_os_str()
            .is_empty()
        {
            self.size = 0;
            return true;
        }

        let mut idx = 0;
        let mut added = false;
        for comp in path
            .components()
            // Basically takes out any '.', since if that thing is at the beginning of the path,
            // this loop is fucked.
            // Why it's fucked if so: it will add '.' to the repo root node, which is stupid.
            .filter(|cp| !matches!(cp, Component::CurDir))
            .map(|c| c.as_os_str())
        {
            // println!("{}", comp.to_string_lossy());
            match self.nodes[idx].get_component(comp) {
                // I will try to find a way to reduce the nesting level. This looks awful.
                None => {
                    match self.next_frees.pop() {
                        None => {
                            self.nodes.push({
                                let mut ret = TreeNode::new(comp);
                                ret.add_parent(idx);
                                ret
                            });
                            self.nodes[idx].add_child(comp, self.size);
                        }
                        Some(Reverse(s)) => {
                            self.nodes[s] = {
                                let mut ret = TreeNode::new(comp);
                                ret.add_parent(idx);
                                ret
                            };
                            self.nodes[idx].add_child(comp, s);
                        }
                    };
                    idx = self.size;
                    self.size += 1;
                    added = true;
                    continue;
                }
                Some(i) => {
                    idx = i;
                }
            }
            if self.nodes[idx].is_leaf() {
                return false;
            }
        }
        // fuck you borrow-checker.
        let to_clear: Vec<usize> = self.nodes[idx].children.iter().map(|c| *c.1).collect();
        for child in to_clear {
            self.next_frees.push(Reverse(child));
            self.nodes[child].filename.clear();
        }
        self.size -= self.nodes[idx].remove_children();

        added
    }

    fn only_repo_root(&self) -> bool {
        // read the comment in `Tree`
        self.size == 0
    }

    fn relative_path(&self, node: &TreeNode) -> PathBuf {
        let mut curr_node = node;
        let mut str_buf = PathBuf::new();
        str_buf.push(&node.filename);
        while let Some(p) = curr_node.parent {
            if p == 0 {
                break;
            }
            curr_node = &self.nodes[p];
            str_buf = Path::join(Path::new(&curr_node.filename), &str_buf);
        }
        str_buf
    }

    /// Create object files from data recorded in this tree.
    ///
    /// # Return values
    /// - Err for any I/O error.
    /// - Ok([u8;20]) otherwise. This is the SHA1 in bytes of the repository root tree.
    pub fn to_object_file(&self) -> Result<[u8; 20]> {
        self.to_object_file_recursive(&self.nodes[0])
    }

    /// Recursive call for `to_object_file`.
    ///
    /// # Return values
    /// - Err for any I/O error.
    /// - Ok([u8;20]) otherwise. This is the SHA1 in bytes of the object represented by the node
    ///   passed in.
    ///
    /// * `node`:
    fn to_object_file_recursive(&self, node: &TreeNode) -> Result<[u8; 20]> {
        let AllPaths {
            dirs_path,
            files_path,
            ..
        } = gyat_paths()?;

        let source_path = self.relative_path(node);
        let mut source_file = File::open(&source_path)?;
        if node.is_leaf() {
            let hash = hash::digest_file(&mut source_file)?;
            source_file.seek(SeekFrom::Start(0))?;
            let blob_content = objects::format_blob_content(&mut source_file)?;

            let blob_path = files_path.join(Path::new(&hash::to_string(&hash)));
            if !blob_path.exists() {
                fs::write(blob_path, blob_content)?;
            }
            return Ok(hash);
        }

        let mut tree_content = String::new();
        for child in &node.children {
            let hash = self.to_object_file_recursive(&self.nodes[*child.1])?;
            let child_type = if self.nodes[*child.1].is_leaf() {
                "blob"
            } else {
                "tree"
            };
            tree_content.push_str(&format!(
                "{}\t{}\t{}\n",
                child_type,
                hash::to_string(&hash),
                Path::new(&self.nodes[*child.1].filename).display()
            ));
        }
        let tree_hash = hash::get_sha1_bytes(tree_content.as_bytes());
        let tree_path = dirs_path.join(Path::new(&hash::to_string(&tree_hash)));

        if !tree_path.exists() {
            fs::write(&tree_path, tree_content)?;
        }

        Ok(tree_hash)
    }
}

#[cfg(test)]
mod test {
    use std::env::current_dir;

    use clap::builder::OsStr;

    use super::*;

    #[test]
    fn init_test() {
        debug_assert!(
            root::is_repo(Path::new(".")),
            "Please run this test inside a .gyat repo"
        );
        let tree = Tree::new().expect("Please run this test inside a .gyat repo");
        assert_eq!(tree.size, 1);
        assert_eq!(&tree.nodes[0], &TreeNode::new(&OsStr::from(".")));
    }

    #[test]
    fn add_root_test() {
        debug_assert!(
            root::is_repo(Path::new(".")),
            "Please run this test inside a .gyat repo"
        );
        let mut tree = Tree::new().expect("Please run this test inside a .gyat repo");
        assert!(!tree.only_repo_root());
        assert!(tree.add_path(Path::new(".")));
        assert!(tree.only_repo_root());
        assert!(!tree.add_path(Path::new("src")));
    }

    #[test]
    fn add_test_1() {
        debug_assert!(
            root::is_repo(Path::new(".")),
            "Please run this test inside a .gyat repo"
        );
        let mut tree = Tree::new().expect("Please run this test inside a .gyat repo");
        assert!(tree.add_path(Path::new("src")));
        assert!(tree.contains_path(Path::new("src")));
        assert!(tree.add_path(Path::new("test-data")));
        assert!(tree.contains_path(Path::new("test-data")));
        assert!(!tree.add_path(Path::new("src/cli.rs")));
        assert!(tree.add_path(Path::new(".")));
        assert!(!tree.contains_path(Path::new("src")));
    }

    #[test]
    fn add_test_2() {
        debug_assert!(
            root::is_repo(Path::new(".")),
            "Please run this test inside a .gyat repo"
        );
        let mut tree = Tree::new().expect("Please run this test inside a .gyat repo");
        assert!(tree.add_path(Path::new("src/cli.rs")));
        assert!(!tree.add_path(Path::new("src")));
        assert!(tree.contains_path(Path::new("src/cli.rs")));
        assert!(tree.contains_path(Path::new("src")));
        // I forgot to test absolute path, so here you go.
        assert!(tree.contains_path(&Path::join(&current_dir().unwrap(), "src")));
    }

    #[test]
    fn leaves_test() {
        debug_assert!(
            root::is_repo(Path::new(".")),
            "Please run this test inside a .gyat repo"
        );
        let mut tree = Tree::new().expect("Please run this test inside a .gyat repo");
        assert!(tree.add_path(Path::new("src/cli.rs")));
        assert!(tree.add_path(Path::new("src/hash.rs")));
        assert!(tree.add_path(Path::new("test-data")));
        // println!("{:#?}", tree.nodes);
        for leaf in tree.leaves() {
            println!("{}", leaf.display());
        }
    }
}
