use clap::ValueEnum;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process,
};

#[derive(Clone, Copy, ValueEnum)]
pub enum Action {
    Report,
    Hardlink,
    Delete,
}

pub fn apply(action: Action, groups: &mut [Vec<PathBuf>]) {
    if groups.is_empty() {
        println!("no duplicates found");
        return;
    }

    // Deterministic keeper: sort each group, first path wins.
    // Also gives stable output ordering across runs.
    for group in groups.iter_mut() {
        group.sort();
    }

    match action {
        Action::Report => report(groups),
        Action::Hardlink => hardlink(groups),
        Action::Delete => delete(groups),
    }
}

fn report(groups: &[Vec<PathBuf>]) {
    for group in groups {
        println!("{} identical files:", group.len());
        for path in group {
            println!("  {}", path.display());
        }
        println!();
    }
}

fn delete(groups: &[Vec<PathBuf>]) {
    let mut removed = 0usize;
    for group in groups {
        let (keeper, dups) = group.split_first().expect("groups always have >1 member");
        for dup in dups {
            match fs::remove_file(dup) {
                Ok(()) => {
                    println!("deleted {} (kept {})", dup.display(), keeper.display());
                    removed += 1;
                }
                Err(e) => eprintln!("could not delete {}: {e}", dup.display()),
            }
        }
    }
    println!("removed {removed} duplicate files");
}

fn hardlink(groups: &[Vec<PathBuf>]) {
    let mut linked = 0usize;
    for group in groups {
        let (keeper, dups) = group.split_first().expect("groups always have >1 member");
        for dup in dups {
            match link_over(keeper, dup) {
                Ok(()) => {
                    println!("linked {} -> {}", dup.display(), keeper.display());
                    linked += 1;
                }
                Err(e) => eprintln!("could not link {}: {e}", dup.display()),
            }
        }
    }
    println!("replaced {linked} duplicates with hardlinks");
}

/// Atomically replace `dup` with a hardlink to `keeper`.
///
/// `fs::hard_link` fails if the destination exists, and
/// remove-then-link leaves a window where a crash loses `dup`
/// entirely. So: link the keeper to a temp name in `dup`'s
/// directory, then rename the temp over `dup`. rename(2) atomically
/// replaces the destination, so `dup` always resolves to either the
/// old content or the new link — never to nothing.
fn link_over(keeper: &Path, dup: &Path) -> io::Result<()> {
    let dir = dup.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!(".dupe-tmp-{}", process::id()));

    fs::hard_link(keeper, &tmp)?;

    if let Err(e) = fs::rename(&tmp, dup) {
        let _ = fs::remove_file(&tmp); // best effort cleanup
        return Err(e);
    }

    Ok(())
}
