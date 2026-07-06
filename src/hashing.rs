use std::{
    collections::HashMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::mpsc,
};

use crate::thread_pool::ThreadPool;

#[derive(Clone, Copy)]
pub enum HashMode {
    /// Hash only the first N bytes.
    Prefix(u64),
    /// Hash the entire file.
    Full,
}

/// Grouping key: content hash qualified by file length.
#[derive(PartialEq, Eq, Hash)]
pub struct FileKey {
    pub len: u64,
    pub hash: [u8; 32],
}

struct HashResult {
    len: u64,
    path: PathBuf,
    hash: [u8; 32],
}

/// Hash `files` on the pool, group by (len, hash), keep groups with >1 member.
pub fn hash_groups(
    pool: &ThreadPool,
    files: impl IntoIterator<Item = (u64, PathBuf)>,
    mode: HashMode,
) -> HashMap<FileKey, Vec<PathBuf>> {
    let (tx, rx) = mpsc::channel::<HashResult>();

    for (len, path) in files {
        let tx = tx.clone();
        pool.execute(move || match hash_file(&path, mode) {
            Ok(hash) => {
                tx.send(HashResult { len, path, hash }).ok();
            }
            Err(e) => eprintln!("could not hash {}: {e}", path.display()),
        });
    }

    drop(tx);

    let mut groups: HashMap<FileKey, Vec<PathBuf>> = HashMap::new();
    for r in rx {
        groups
            .entry(FileKey {
                len: r.len,
                hash: r.hash,
            })
            .or_default()
            .push(r.path);
    }

    groups.retain(|_, v| v.len() > 1);
    groups
}

fn hash_file(path: &Path, mode: HashMode) -> io::Result<[u8; 32]> {
    let fd = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    match mode {
        HashMode::Prefix(n) => {
            io::copy(&mut fd.take(n), &mut hasher)?;
        }
        HashMode::Full => {
            hasher.update_reader(fd)?;
        }
    }
    Ok(hasher.finalize().into())
}
