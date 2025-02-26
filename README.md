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


