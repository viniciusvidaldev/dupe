# dupe

A small command line tool that finds duplicate files under a directory and either reports them, replaces them with hardlinks, or deletes the extras.

Written as a Rust exercise. Unix only (it uses inode numbers to skip hardlinks).

## How it works

1. Walk the directory recursively and group files by size.
2. Drop every group with a single file. Different size means different content.
3. For each remaining group, hash the first 4096 bytes with BLAKE3 in a thread pool.
4. Files smaller than the prefix are already fully hashed and confirmed as duplicates.
5. Anything still ambiguous gets a full content hash.
6. Apply the chosen action to each confirmed group.

Hardlinks pointing to the same inode are detected early and skipped, so the same file appearing twice on disk isn't treated as a duplicate.

## Build

```
cargo build --release
```

## Usage

```
dupe <path> --action <report|hardlink|delete>
```

The action is required. There is no default. You have to say what you want to happen.

### Report

Prints each group of identical files without touching anything.

```
dupe ~/Downloads --action report
```

### Hardlink

Keeps the first file in each group (sorted alphabetically) and replaces the others with hardlinks to it. The replacement is atomic: it links the keeper to a temp name in the target's directory, then renames over the duplicate. A crash mid operation never leaves the duplicate missing.

```
dupe ~/Downloads --action hardlink
```

### Delete

Keeps the first file in each group and removes the rest.

```
dupe ~/Downloads --action delete
```

## Notes

The thread count comes from `std::thread::available_parallelism`, capped by the number of candidate files. If the syscall fails it falls back to 4.

BLAKE3 was chosen because it's fast and has a good streaming API.
