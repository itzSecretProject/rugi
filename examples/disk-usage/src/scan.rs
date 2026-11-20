//! Walks a folder on a background thread and builds a tree of sizes.
//!
//! Sizes are the space allocated on disk where the platform reports it (Unix
//! block counts), otherwise the file length. Hard-linked files count once,
//! symbolic links are not followed, and the walk stays on the file system of
//! the folder it starts in.

use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Folder,
    File,
    Link,
}

/// A file or folder with the totals of everything below it.
#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub kind: Kind,
    /// Bytes on disk, including everything below a folder.
    pub size: u64,
    /// Files and links below a folder (1 for a file).
    pub files: u64,
    pub children: Vec<Node>,
    /// Why the folder could not be read completely.
    pub error: Option<String>,
    /// Entries at or below this folder that could not be read.
    pub unreadable: u64,
}

/// Counters the UI reads while a scan runs.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    /// The folder being read.
    pub current: Mutex<PathBuf>,
    stop: AtomicBool,
}

pub struct Scan {
    pub root: PathBuf,
    pub progress: Arc<Progress>,
    started: Instant,
    thread: Option<JoinHandle<io::Result<Node>>>,
}

/// A completed (or stopped) scan.
pub struct Finished {
    pub tree: io::Result<Node>,
    pub elapsed: Duration,
    /// Stopped by the user: totals are incomplete.
    pub stopped: bool,
    pub errors: u64,
}

impl Scan {
    pub fn start(root: PathBuf) -> Self {
        let progress = Arc::new(Progress::default());
        let (path, p) = (root.clone(), Arc::clone(&progress));
        let thread = std::thread::spawn(move || scan_root(&path, &p));
        Self { root, progress, started: Instant::now(), thread: Some(thread) }
    }

    /// Asks the walk to stop; the folders read so far are kept.
    pub fn stop(&self) {
        self.progress.stop.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// The result, once the walk has ended.
    pub fn poll(&mut self) -> Option<Finished> {
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        let tree = match self.thread.take()?.join() {
            Ok(tree) => tree,
            Err(_) => Err(io::Error::other("the scan thread panicked")),
        };
        let p = &self.progress;
        Some(Finished {
            tree,
            elapsed: self.started.elapsed(),
            stopped: p.stop.load(Ordering::Relaxed),
            errors: p.errors.load(Ordering::Relaxed),
        })
    }
}

struct Walk<'a> {
    progress: &'a Progress,
    device: Option<u64>,
    /// (device, inode) of hard-linked files already counted.
    seen: HashSet<(u64, u64)>,
}

fn scan_root(root: &Path, progress: &Progress) -> io::Result<Node> {
    let meta = fs::symlink_metadata(root)?;
    let name = root.display().to_string();
    let mut walk = Walk { progress, device: device(&meta), seen: HashSet::new() };
    Ok(if meta.is_dir() { walk.folder(root, name) } else { walk.leaf(name, &meta) })
}

impl Walk<'_> {
    fn folder(&mut self, path: &Path, name: String) -> Node {
        if let Ok(mut current) = self.progress.current.lock() {
            *current = path.to_path_buf();
        }
        let mut node =
            Node { name, kind: Kind::Folder, size: 0, files: 0, children: Vec::new(), error: None, unreadable: 0 };
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) => {
                self.failed(&mut node, &e);
                return node;
            }
        };
        for entry in entries {
            if self.progress.stop.load(Ordering::Relaxed) {
                break;
            }
            let (entry, meta) = match entry.and_then(|e| e.metadata().map(|m| (e, m))) {
                Ok(pair) => pair,
                Err(e) => {
                    self.failed(&mut node, &e);
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if meta.is_dir() && device(&meta) == self.device {
                self.folder(&entry.path(), name)
            } else {
                self.leaf(name, &meta)
            };
            node.size += child.size;
            node.files += child.files;
            node.unreadable += child.unreadable;
            node.children.push(child);
        }
        node
    }

    fn leaf(&mut self, name: String, meta: &Metadata) -> Node {
        let kind = if meta.is_symlink() {
            Kind::Link
        } else if meta.is_dir() {
            // A mount point: listed, not entered.
            Kind::Folder
        } else {
            Kind::File
        };
        let size = if self.first_link(meta) { allocated(meta) } else { 0 };
        let files = u64::from(kind != Kind::Folder);
        self.progress.files.fetch_add(files, Ordering::Relaxed);
        self.progress.bytes.fetch_add(size, Ordering::Relaxed);
        let error = (kind == Kind::Folder).then(|| "On another file system; not scanned".to_string());
        Node { name, kind, size, files, children: Vec::new(), error, unreadable: 0 }
    }

    fn failed(&self, node: &mut Node, e: &io::Error) {
        self.progress.errors.fetch_add(1, Ordering::Relaxed);
        node.unreadable += 1;
        node.error.get_or_insert_with(|| e.to_string());
    }

    /// False for a hard link to a file that was already counted.
    #[cfg(unix)]
    fn first_link(&mut self, meta: &Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() <= 1 || meta.is_dir() || self.seen.insert((meta.dev(), meta.ino()))
    }

    #[cfg(not(unix))]
    fn first_link(&mut self, _meta: &Metadata) -> bool {
        true
    }
}

#[cfg(unix)]
fn allocated(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn allocated(meta: &Metadata) -> u64 {
    meta.len()
}

#[cfg(unix)]
fn device(meta: &Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.dev())
}

#[cfg(not(unix))]
fn device(_meta: &Metadata) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_up() {
        let dir = std::env::temp_dir().join(format!("rugi-disk-usage-{}", std::process::id()));
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.bin"), vec![1u8; 10_000]).unwrap();
        fs::write(dir.join("sub/b.bin"), vec![1u8; 20_000]).unwrap();
        let progress = Progress::default();
        let tree = scan_root(&dir, &progress).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(tree.kind, Kind::Folder);
        assert_eq!(tree.files, 2);
        assert_eq!(tree.children.len(), 2);
        let sub = tree.children.iter().find(|c| c.name == "sub").unwrap();
        assert_eq!(sub.files, 1);
        assert!(sub.size >= 20_000);
        assert_eq!(tree.size, tree.children.iter().map(|c| c.size).sum::<u64>());
        assert_eq!(progress.files.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn missing_folder_is_an_error() {
        assert!(scan_root(Path::new("/definitely/not/here"), &Progress::default()).is_err());
    }
}
