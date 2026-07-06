mod actions;
mod hashing;
mod thread_pool;
mod walker;

use clap::Parser;
use std::{
    collections::{HashMap, HashSet},
    fs,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    process::ExitCode,
    thread,
};

use crate::{
    actions::Action,
    hashing::{HashMode, hash_groups},
    thread_pool::ThreadPool,
    walker::FileWalker,
};

const DEFAULT_NUM_THREADS: usize = 4;
const PREFIX_LEN: u64 = 4096;

#[derive(Parser)]
struct Args {
    /// Root directory to scan
    path: PathBuf,

    /// What to do with confirmed duplicates (must choose explicitly)
    #[arg(long, value_enum)]
    action: Action,
}

fn main() -> ExitCode {
    let args = Args::parse();

    match fs::metadata(&args.path) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            eprintln!("{} is not a directory", args.path.display());
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("could not open {}: {e}", args.path.display());
            return ExitCode::FAILURE;
        }
    }

    eprintln!("warning: do not modify files in the scanned directories while dupe is running");

    let walker = FileWalker::new(args.path);

    let mut map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();

    for file in walker {
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                eprintln!("failed to read entry: {e}");
                continue;
            }
        };
        // Follow symlinks: we want the target's inode and size,
        // not the symlink's. DirEntry::metadata uses lstat.
        let meta = match fs::metadata(file.path()) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("could not read {} metadata: {e}", file.path().display());
                continue;
            }
        };

        // same inode = same file; a hardlink is not a duplicate
        if !seen_inodes.insert((meta.dev(), meta.ino())) {
            continue;
        }

        if meta.len() == 0 {
            continue;
        }

        map.entry(meta.len()).or_default().push(file.path());
    }

    // rule out files with unique size
    map.retain(|_, v| v.len() > 1);
    if map.is_empty() {
        println!("no duplicates found");
        return ExitCode::SUCCESS;
    }

    let num_threads = match thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => {
            eprintln!("could not get available parallelism, using default: {DEFAULT_NUM_THREADS}");
            DEFAULT_NUM_THREADS
        }
    };

    let total_candidates = map.values().map(Vec::len).sum();
    let pool = ThreadPool::new(num_threads.min(total_candidates));

    let candidates = map
        .into_iter()
        .flat_map(|(len, paths)| paths.into_iter().map(move |p| (len, p)));

    let by_prefix = hash_groups(&pool, candidates, HashMode::Prefix(PREFIX_LEN));

    // files no larger than the prefix are already fully hashed
    let mut confirmed: Vec<Vec<PathBuf>> = Vec::new();
    let mut pending: Vec<(u64, PathBuf)> = Vec::new();
    for (key, paths) in by_prefix {
        if key.len <= PREFIX_LEN {
            confirmed.push(paths);
        } else {
            pending.extend(paths.into_iter().map(|p| (key.len, p)));
        }
    }

    // full hash for everything still ambiguous
    let by_content = hash_groups(&pool, pending, HashMode::Full);
    confirmed.extend(by_content.into_values());
    actions::apply(args.action, &mut confirmed);

    ExitCode::SUCCESS
}
