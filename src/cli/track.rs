use std::{fmt::Write, fs::File, path::Path};

use super::observe;
use crate::Result;
use chrono::{DateTime, Local};
use gyat::{
    dirtree::Tree,
    fs, hash::{self, get_sha1_string},
    objects::{self, CommitObject},
    utils,
};

pub fn track(message: &Option<String>, track_all: bool) -> Result<()> {
    let utils::AllPaths {
        head_path,
        index_path,
        commits_path,
        ..
    } = utils::gyat_paths()?;
    if track_all {
        observe::observe(&[std::path::PathBuf::from(".")])?;
    }

    let observed_list = fs::read_index(&mut File::open(&index_path)?)?;
    if observed_list.is_empty() {
        println!("No changes found");
        return Ok(());
    }
    let parent_commit = match std::fs::read_to_string(&head_path) {
        Ok(content) if !content.trim().is_empty() => Some(content.trim().to_string()),
        _ => None,
    };

    let mut dtree = Tree::new()?;
    if let Some(pc) = &parent_commit {
        let pc_hash = hash::from_string(pc).unwrap();
        let CommitObject { root, .. } = objects::read_commit_content(&pc_hash).unwrap();
        let mut prev_blobs = objects::get_blobs_from_root(&root)?;
        for entry in &observed_list {
            use fs::ChangeType::*;
            match entry.change {
                New => {
                    dtree.add_path(&entry.path);
                }
                Mod => {
                    dtree.add_path(&entry.path);
                    prev_blobs.remove(&entry.path);
                }
                Del => {
                    prev_blobs.remove(&entry.path);
                }
            }
        }
        for blob_left in prev_blobs {
            dtree.add_path(&blob_left.0);
        }
    } else {
        for entry in &observed_list {
            dtree.add_path(&entry.path);
        }
    }

    let root_hash = dtree.to_object_file()?;

    let local_current: DateTime<Local> = Local::now();
    let formatted_date = local_current.format("%a %b %d %H:%M:%S %Y").to_string();
    let commit_message = message.clone().unwrap_or_default();
    let formatted_change_list = observed_list.iter().fold(String::new(), |mut out, ie| {
        let _ = writeln!(out, "{:?}\t{}", ie.change, ie.path.display());
        out
    });
    let commit_content = format!(
        "Parent: {}\nTree: {}\nMessage: {}\nDate: {}\nChanges:\n{}",
        parent_commit.unwrap_or(String::from("0")),
        hash::to_string(&root_hash),
        commit_message,
        formatted_date,
        formatted_change_list
    );
    let commit_hash = get_sha1_string(commit_content.as_bytes());
    std::fs::write(commits_path.join(Path::new(&commit_hash)), commit_content)?;
    std::fs::write(head_path, commit_hash)?;
    std::fs::write(index_path, "")?;

    Ok(())
}
