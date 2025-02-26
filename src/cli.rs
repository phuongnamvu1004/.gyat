use crate::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use clap::{self, Parser, Subcommand};
use gyat::root;

mod create;
mod observe;
mod track;
mod fallback;

/// Watered down VCS
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    /// Runs the program.
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Create { name } => Ok(create::create(name)?),
            Command::Observe { paths } => Ok(observe::observe(paths)?),
            Command::Track { message, track_all } => Ok(track::track(message, *track_all)?),
            Command::Fallback { commit_hash } => Ok(fallback::fallback(commit_hash.as_ref())?),
            Command::Wood { lines } => Ok(Self::wood(*lines)?),
        }
    }

    /// Prints out a log of commit hashes, for now.
    ///
    /// * `lines`:
    fn wood(lines: usize) -> Result<()> {
        if lines == 0 {
            return Ok(());
        }

        let repo_root = root::get_repo_root(std::env::current_dir()?.as_path())
            .ok_or("Current directory in not in gyat repositroy")?;
        let gyat_path = repo_root.join(".gyat");

        if !gyat_path.exists() {
            return Err("Repository is not created".into());
        }

        let mut curr_commit = {
            let mut reader = BufReader::new(File::open(gyat_path.join("HEAD"))?);
            let mut buf = String::new();
            reader.read_line(&mut buf)?;
            buf
        };
        println!("{}", curr_commit.trim());

        let commits_path = gyat_path.join("commits");
        for _ in 1..lines {
            let curr_commit_file =
                File::open(commits_path.join(Path::new(&curr_commit))).map_err(|e| {
                    format!(
                        "{}: {e}",
                        &commits_path.join(Path::new(&curr_commit)).display()
                    )
                })?;
            let mut reader = BufReader::new(curr_commit_file);
            curr_commit.clear();
            reader.read_line(&mut curr_commit)?;
            curr_commit = curr_commit.split(':').nth(1).unwrap().trim().to_owned();
            if curr_commit.is_empty() {
                return Ok(());
            }
            if curr_commit.len() < 20 {
                return Ok(());
            }
            println!("{}", curr_commit.trim());
        }

        Ok(())
    }
}

#[derive(Subcommand)]
/// Valid subcommands
enum Command {
    /// Create a new gyat repository.
    Create {
        /// The name of the repository (hence the directory name). If this option is not supplied,
        /// create the repository in the current directory instead.
        name: Option<String>,
    },
    /// Take a look at the repository for changes.
    /// Use . to track all files in the current working directory.
    Observe {
        /// The list of files to observe.
        /// This can also be a list of directories,
        /// in which case all files in those directories are tracked.
        #[arg(short, long, default_value = ".", num_args = 1..)]
        paths: Vec<PathBuf>,
    },
    /// Commit the changes observed.
    Track {
        /// The commit message.
        /// If this option is not used, vim is opened.
        /// Commit messages cannot be empty.
        #[arg(short, long, default_value = None)]
        message: Option<String>,
        /// Equivalent to calling gyat observe before this command.
        #[arg(short = 'a', long)]
        track_all: bool,
    },
    /// Fall back to a previous track
    Fallback {
        /// the hash value of the tracked change (required argument)
        #[arg(required = true)]
        commit_hash: Option<String>,
    },
    // this prints a log of all changes. We may actually implement this right after track
    Wood {
        /// Maximum number of lines to display the log
        #[arg(short = 'n', long, default_value = "10")]
        lines: usize,
    },
}
