# Group project

> Proposed name for now: `gyat`.

## Introduction

A program that imitates `git`'s basic features. In other words, it's a version
control program.

## How are we doing it?

At a high level, we expect out minimally viable product to accept the following
command-line arguments:

> Note: the exact syntax may change

```bash
# new repository
gyat create -- [NAME]

# add changes
# no ignore files considered yet, for now
gyat observe -- [FILES]
gyat observe --help

# commit changes just observed
# also, forces users to use vim to edit commits
gyat track
gyat track --help

# revert changes
# for now, we will just use an auto-incrementing unsigned integer as the hash.
gyat fallback -- [HASH]
gyat fallback --help

# Mister Obvious
gyat --version
gyat --help
```

### How do we track the changes?

At a high level:

When we first create a repository with `gyat create`, we create the first
snapshot of the entire file system.

Whenever we call `gyat observe`, we compare each file to the most recent
snapshots of itself. There are a couple scenarios that could happen:

- The file is the same as before: then it's not changed.
- The file is different than its old snapshot: then it's changed.
- There isn't this file in the snapshot: then it's a new file.
- A snapshot doesn't have a corresponding file: then that file is gone
- A filename is now a directory name: :thinking:

Any file that falls into any scenarios different than the first one will
be stored in a temporary snapshot.

To help with comparing these files, we will use a card from `git`: encrypt
the file's data into `SHA-1`. According to `git` itself, the chance of 2
different files generating the same `SHA-1` is so small, we could consider
that impossible.

To store the snapshot, we can use a compression library to save on storage.

> At this point, one can think of each snapshot as a hash table with key
as the `SHA-1` of a file and the value as the path to the compressed data
of that file.

`gyat track` first checks if there's any temporary snapshot available, and
if yes, adds the contents of that snapshot into the main snapshot.

Tracking may be viewed at a high-level as a directed, acyclic graph. Each node
in the track contains metadata about each track (content of the metadata is
still being discussed). Like above, to more efficiently find and compare
tracks, we create an `SHA-1` key for each node.

> Note: with the no-branch design we're going to implement, it's just a linked
list, but we want to keep our options open.

`gyat fallback` looks for a node in the current track graph corresponding to
the hash given. We will take another card in `git`'s deck here: one can
shorthand the hash as its first characters, and `fallback` falls back to the
first node with the hash whose first characters are the same. The user could
of course use more characters. Once it reaches the very first node of the
graph, but that node still isn't the node it's looking for, it throws an error,
and the reversion is aborted.

During the graph traversal, `fallback` rebuilds each snapshot by looking at the
list of changes in each node.

## What to learn?

- Mostly, how to represent data structures on a file (not on memory).
  - The most notable is probably a hash table. For efficiency, we are thinking
  about a BTree hash table.

- How to use an encryption and compression library.

## Proposed schedule

| Feature | Estimated deadline |
|---------|-------------------|
| **gyat --version** | 10/27 |
| **gyat --help** | constantly updating |
| **gyat create -- [NAME]** | 11/03 |
| **gyat observe -- [FILES]** | 11/10 |
| **gyat observe --help** | 11/17 |
| **gyat track** | 11/24 |
| **gyat track --help** | 01/12 |
| **gyat fallback -- [HASH]** | 08/12 |
| **gyat fallback --help** | 15/12 |

## Status update (as of 11/15)

After several weeks of development, we've made some scope adjustments to ensure
project completion while maintaining essential features.

## Current status

### What we have successfully implemented

- Basic repository creation `gyat create`
- Framework for creating `gyat observe`
- Finding the crate of SHA1 implementation

### Challenges encounter

**Technical Challenges:**

- File system interactions more complex than anticipated
- SHA-1 implementation requiring additional security considerations

Generally, at a high level, we still continue with our original idea. However,
there are multiple updates on the implementation details of each functionality
of `gyat`:

## gyat create

> **Status**: Finished minimum viable version for `gyat create`, added to
`main` branch.

### Repository Name Validation

- **With provided name:**
  - Check for invalid names (empty, `.`, `..`)
  - Create `PathBuf` instance from the given path
- **Without name:**
  - Use current working directory

### Directory Creation

- **For named repositories:**
  - Create directory if it doesn't exist
  - Return error if path exists but is not a directory (e.g., file, link,...)

### Repository Structure

- Create `.gyat` directory
  - Return error if `.gyat` already exists in target path
- Initialize internal structure:
  - `commits/` to store commit objects
  - `dirs/` to store tree objects
  - `files/` to store blob objects
  - `HEAD` file for current state tracking

## gyat observe

> **Status**: Planning stage

### Core Dependencies

- SHA1 hash generation
  - Required for unique object identification
  - Must be consistent across identical content
- Compression functionality (optional)
  - For minimizing storage footprint
  - Similar to zlib implementation

### Working Directory Snapshot

- **Directory Traversal:**
  - Algorithm to scan working directory structure
  - Handle nested directories recursively
  - Generate SHA1 keys for each file/directory
- **Path Handling:**
  - Process supplied directory argument if provided
  - Default to current working directory
  - Verify path is within repository scope

### Object File Structure

- **Tree Objects:**
  - Define format for directory representation
  - Must maintain SHA1 consistency for unchanged content
  - Store references to contained files and subdirectories
- **Blob Objects:**
  - Define format for file content storage
  - Ensure identical content produces identical SHA1
  - Handle file content compression

### Parent Directory Updates

- **For Subdirectory Changes:**
  - Update parent directory references
  - Maintain tree object hierarchy
  - Skip parent updates for repository root

### Implementation Requirements

- Longest common substring algorithm adaptation
  - For efficient change detection
  - Compare file/directory modifications
- Consistent object formatting
  - Define clear structure for trees and blobs
  - Ensure reproducible SHA1 generation

## gyat track

### Core Dependencies

- `gyat track` must be executed after `gyat observe`.
- The result of `gyat observe` is an "intermediary" commit object containing:
  - A commit message (provided by the user).
  - Temporary tree and blob objects representing the snapshot.

### Commit Object Creation:

- The command converts the intermediary commit object into a finalized commit.
- Updates metadata about the commit (e.g., timestamp, parent commit).

### Tree Traversal for Change Detection:

- **Blob Objects:**
  - Traverse tree objects to locate blobs.
  - Determine the file's path in the repository.
- **File status checks:**
  - If the file does not exist in the working directory: Mark it as deleted.
  - If the file exists: Convert it into its SHA-1 hash.
    - If the hash matches the tree object: No change.
    - If the hash differs: Mark as modified.

## gyat fallback

### Overview:

- Reverts the repository to a snapshot identified by a hash.
- Traverses the snapshot history.
- Rebuilds the working directory based on the snapshot.

### Steps:

- check whether the .gyat directory exists.
- Start from head and move backward to find the matching node.
- check whether hash is valid and matched a node
- cast an error if there is no matching node
- Clear the current working directory except .gyat.
- Use snapshot data from .gyat/: Tree and Blob objects
- ​Update head to the reverted snapshot.

## gyat --version

## gyat --help

## Revised schedule

| Feature | Old deadline | New deadline|
|---------|-------------------|-----------|
| **gyat --version** | ~~10/27~~ | 11/17|
| **gyat --help** | constantly updating | constantly updating|
| **gyat create -- [NAME]** | 11/03 | ✅|
| **gyat observe -- [FILES]** | ~~11/10~~ | 11/24|
| **gyat observe --help** | ~~11/17~~ | 11/24|
| **gyat track** | ~~11/24~~ | 12/01|
| **gyat track --help** | ~~12/01~~ | 12/01|
| **gyat fallback -- [HASH]** | 12/08 | 12/08|
| **gyat fallback --help** | ~~12/15~~ | 12/08|
| **Project Finalization** | None | 12/15|
