use std::fs::File;
use std::path::Path;
use std::{collections::HashMap, env::current_dir, path::PathBuf};
use gyat::{
    fs, hash, objects
};

use std::fs::create_dir_all;
use std::fs::remove_file;
use std::fs::remove_dir;

use crate::cli::observe::observe;
use crate::cli::track::track;

use crate::Result;

/// - Requirement: 
///     + The argument (specific commit that we want to get to) is provided
///     + There must be a previous commit
/// Behaviour:
/// - Gets the root tree hash from the target commit
/// - Gets all blobs (files) from that commit's tree
/// - Cleans up the working directory by removing files that aren't in the target commit
/// - Creates or updates files based on the target commit's blobs
/// - Updates HEAD to point to the checked-out commit
pub fn fallback(commit_hash: Option<&String>) -> Result<()> {
    let repo_path = current_dir()?;
    let gyat_path = repo_path.join(".gyat");

    let head_blobs = match get_blobs_from_head(&gyat_path) {
        Ok(blobs) => blobs,
        Err(_) => return Ok(()) 
    };

    let commit_blobs = match get_blobs_from_commit(&gyat_path, commit_hash) {
        Ok(blobs) => blobs,
        Err(_) => return Ok(()) 
    };

    let changes = compare_trees(head_blobs, commit_blobs).unwrap();

    process_change(&changes)?;

    observe(&[PathBuf::from(".")])?;
    track(&Some(format!("Fallback to the commit with commit_id {}", commit_hash.unwrap()).to_string()), true)?;

    log_fallback_action(commit_hash.unwrap(), changes)?;

    Ok(())
}

fn get_blobs_from_head(gyat_path: &PathBuf) -> Result<HashMap<PathBuf, [u8; 20]>> {
    if let Some(head_root) = fs::get_root_tree_hash(gyat_path, None)? {
        // Get all blobs from the lastest commit's root tree
        let head_blobs = objects::get_blobs_from_root(&hash::from_string(&head_root).unwrap())?;
        
        Ok(head_blobs)
    } else {
        Err("There is no previous commit".into())
    }
}

fn get_blobs_from_commit(gyat_path: &PathBuf, commit_hash: Option<&String>) -> Result<HashMap<PathBuf, [u8; 20]>> {
    if let Some(commit_root) = fs::get_root_tree_hash(gyat_path, commit_hash)? {
        // Get all blobs from the specified commit's root tree
        let commit_blobs = objects::get_blobs_from_root(&hash::from_string(&commit_root).unwrap())?;
        
        Ok(commit_blobs)
    } else {
        Err("There is no such commit".into())
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct Changes {
    to_add: Vec<(PathBuf, [u8; 20])>,
    to_modify: Vec<(PathBuf, [u8; 20])>,
    to_delete: Vec<PathBuf>,
}

fn compare_trees(head_blobs: HashMap<PathBuf, [u8; 20]>, commit_blobs: HashMap<PathBuf, [u8; 20]>) -> Result<Changes> {
    let mut changes = Changes {
        to_add: Vec::new(),
        to_modify: Vec::new(),
        to_delete: Vec::new(),
    };

    // Find files that need to be added back (exist in the specified commit but not in HEAD anymore) for remodified  
    for (path, commit_hash) in commit_blobs.iter() {
        match head_blobs.get(path) {
            Some(head_hash) => {
                // File exists in both commits
                if head_hash != commit_hash {
                    // Hash is different, so file was modified
                    changes.to_modify.push((path.clone(), *commit_hash));
                }
            }
            None => {
                // File only exists in target commit, so we need to add it back
                changes.to_add.push((path.clone(), *commit_hash));
            }
        }
    }

    // Find files that needed to be deleted
    for (path, _head_hash) in head_blobs.iter() {
        if !commit_blobs.contains_key(path) {
            // File exists in HEAD but not in target commit, so it is to delete
            changes.to_delete.push(path.clone());
        }
    }

    Ok(changes)
}

fn process_change(changes: &Changes) -> Result<()> {
    // Process added and modified files
    for (path, hash) in &changes.to_add {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        // Create empty file and write the content
        File::create(path)?;
        let content = objects::read_blob(hash)?;
        std::fs::write(path, content)?;
    }

    // Both added and modified files need their contents updated
    for (path, hash) in &changes.to_modify {
        // Read blob content from object store
        let content = objects::read_blob(hash)?;
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        
        // Write content to file
        File::create(path)?;
        std::fs::write(path, content)?;
    }

    // Remove deleted files
    for path in &changes.to_delete {
        // Check if file exists before attempting to remove
        if path.exists() {
            remove_file(path)?;
            
            // Try to remove empty parent directories
            cleanup_empty_dirs(path.parent())?;
        }
    }

    Ok(())
}

// Helper function to recursively remove empty directories
fn cleanup_empty_dirs(dir: Option<&Path>) -> Result<()> {
    let Some(dir) = dir else {
        return Ok(());
    };

    // Try to remove directory and continue with parent if successful
    match remove_dir(dir) {
        Ok(_) => cleanup_empty_dirs(dir.parent())?,
        Err(_) => () // Directory not empty or already removed
    }

    Ok(())
}

fn log_fallback_action(commit_id: &String, changes: Changes) -> Result<()> {
    // Implementation for logging the action taken
    println!("Fallback to commit {}", commit_id);
    println!("Added files: {:?}", changes.to_add);
    println!("Modified files: {:?}", changes.to_modify);
    println!("Deleted files: {:?}", changes.to_delete);
    Ok(())
}
