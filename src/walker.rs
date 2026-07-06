use std::{fs, io, path::PathBuf};

pub struct FileWalker {
    root: Option<PathBuf>,
    stack: Vec<fs::ReadDir>,
}

impl FileWalker {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
            stack: Vec::new(),
        }
    }
}

impl Iterator for FileWalker {
    type Item = io::Result<fs::DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.take() {
            match fs::read_dir(root) {
                Ok(rd) => self.stack.push(rd),
                Err(e) => return Some(Err(e)),
            };
        }

        while let Some(rd) = self.stack.last_mut() {
            let Some(entry) = rd.next() else {
                self.stack.pop(); // last dir exhausted
                continue;
            };

            let entry = match entry {
                Ok(e) => e,
                Err(e) => return Some(Err(e)),
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => return Some(Err(e)),
            };

            if file_type.is_dir() {
                match fs::read_dir(entry.path()) {
                    Ok(rd) => self.stack.push(rd),
                    Err(e) => return Some(Err(e)),
                }

                continue;
            }

            if file_type.is_file() {
                return Some(Ok(entry));
            }

            // Symlinks: resolve the target and include it if it points
            // at a real file. We never traverse into symlinked
            // directories, to keep the walk loop-free without having
            // to track visited inodes across the whole scan.
            if file_type.is_symlink() {
                match fs::metadata(entry.path()) {
                    Ok(m) if m.is_file() => return Some(Ok(entry)),
                    Ok(_) => continue, // dir or other, skip
                    Err(e) => {
                        eprintln!("skipping broken symlink {}: {e}", entry.path().display());
                        continue;
                    }
                }
            }

            continue; // some entry that isn't a file, dir, or symlink
        }

        None
    }
}
