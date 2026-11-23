//! The publisher's resource map: immutable snapshots of the current page, behind a port.
//!
//! Callers: the provider adapter and local publication tools. A `get` locates one snapshot of the current resource; each snapshot is an immutable value pinned by
//! reference counting, so replacing or removing a resource while a response is in flight never mixes old and new bytes. Nothing here lets a peer publish, edit or
//! list: only an authorized local administrator replaces or removes resources. A path is a logical name, never permission to read an arbitrary file: the directory
//! adapter confines every lookup under its root and refuses symbolic links.
use super::terms::Terms;
use crate::modules::pages::{error::Error, media::MediaType, path::CanonicalPath};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

/// One immutable resource snapshot: its media type and exact bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The resource's media type.
    pub media_type: MediaType,
    /// The exact bytes, shared so an in-flight response keeps them even after the resource changes.
    pub content: Arc<Vec<u8>>,
}

/// Where the current resources come from.
pub trait Pages: Send + Sync {
    /// One snapshot of the resource at `path`, or `None` when it has no current resource.
    /// # Errors
    /// `Storage` when the source cannot be read.
    fn snapshot(&self, path: &CanonicalPath) -> Result<Option<Snapshot>, Error>;

    /// Whether the root exists, is `text/html` and fits the offering's resource limit: the condition for advertising an offering.
    /// # Errors
    /// `Invalid` when the root is missing, not HTML or too large; `Storage` when the source cannot be read.
    fn check_root(&self, terms: &Terms) -> Result<(), Error> {
        tracing::trace!("Checking the page root against an offering");
        let root = self
            .snapshot(&CanonicalPath::root())?
            .ok_or(Error::Invalid)?;
        if root.media_type.is_html() && root.content.len() as u64 <= terms.maximum_resource_bytes {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
}

/// An in-memory resource map, replaced and removed by local tools.
#[derive(Default)]
pub struct MemoryPages {
    map: RwLock<BTreeMap<CanonicalPath, Snapshot>>,
}

impl MemoryPages {
    /// Publish or replace the resource at `path` with a complete new snapshot.
    pub fn publish(&self, path: CanonicalPath, media_type: MediaType, content: Vec<u8>) {
        tracing::trace!("Publishing a page resource locally");
        let snapshot = Snapshot {
            media_type,
            content: Arc::new(content),
        };
        self.map
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path, snapshot);
    }

    /// Remove the resource at `path`, if any.
    pub fn remove(&self, path: &CanonicalPath) {
        tracing::trace!("Removing a page resource locally");
        self.map
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(path);
    }
}

impl Pages for MemoryPages {
    // A clone of the snapshot handle: the bytes themselves are shared and never mutated.
    fn snapshot(&self, path: &CanonicalPath) -> Result<Option<Snapshot>, Error> {
        tracing::trace!("Reading a page resource snapshot");
        Ok(self
            .map
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .cloned())
    }
}

/// A resource map backed by a directory: the root path `/` is `index.html` and `/a/b` is the file `a/b` under the root. Lookups are confined to the root and
/// refuse symbolic links at every component, so no peer-supplied path reaches outside it.
pub struct DirectoryPages {
    root: PathBuf,
    maximum_bytes: u64,
}

impl DirectoryPages {
    /// A directory source reading at most `maximum_bytes` per resource.
    pub fn new(root: PathBuf, maximum_bytes: u64) -> Self {
        tracing::trace!("Creating a confined directory page source");
        Self {
            root,
            maximum_bytes,
        }
    }

    // The file a path names, with every component checked to be a plain directory entry and not a link.
    fn file_of(&self, path: &CanonicalPath) -> Option<PathBuf> {
        let relative = if path.is_root() {
            "index.html"
        } else {
            &path.as_str()[1..]
        };
        let mut file = self.root.clone();
        for component in relative.split('/') {
            file.push(component);
            let meta = std::fs::symlink_metadata(&file).ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
        }
        std::fs::symlink_metadata(&file)
            .ok()
            .filter(|meta| meta.is_file())
            .map(|_| file)
    }
}

impl Pages for DirectoryPages {
    // Read one confined regular file at most once into an immutable snapshot, refusing anything larger than the configured bound.
    fn snapshot(&self, path: &CanonicalPath) -> Result<Option<Snapshot>, Error> {
        tracing::trace!("Reading a confined page file");
        let Some(file) = self.file_of(path) else {
            return Ok(None);
        };
        let length = std::fs::metadata(&file).map_err(|_| Error::Storage)?.len();
        if length > self.maximum_bytes {
            return Err(Error::Limit);
        }
        let content = std::fs::read(&file).map_err(|_| Error::Storage)?;
        Ok(Some(Snapshot {
            media_type: media_type_of(&file),
            content: Arc::new(content),
        }))
    }
}

// The label for a file by its extension; unknown extensions are `application/octet-stream`.
fn media_type_of(file: &Path) -> MediaType {
    let label = match file.extension().and_then(|e| e.to_str()) {
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    MediaType::parse(label).unwrap_or_else(|_| MediaType::octet_stream())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::pages::service::terms::tests::offering;

    fn path(text: &str) -> CanonicalPath {
        CanonicalPath::parse(text.as_bytes()).unwrap()
    }
    fn html() -> MediaType {
        MediaType::parse("text/html").unwrap()
    }
    fn dir(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "ts-pages-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    // PG-01: local publication, replacement and removal change the current resource map; the root rule judges the offering.
    #[test]
    fn local_publication_replaces_and_removes() {
        let pages = MemoryPages::default();
        let terms = Terms::parse(&offering()).unwrap();
        assert_eq!(pages.check_root(&terms), Err(Error::Invalid));
        pages.publish(
            path("/"),
            MediaType::parse("text/plain").unwrap(),
            b"abc".to_vec(),
        );
        assert_eq!(pages.check_root(&terms), Err(Error::Invalid));
        pages.publish(path("/"), html(), vec![1; 101]);
        assert_eq!(pages.check_root(&terms), Err(Error::Invalid));
        pages.publish(path("/"), html(), b"abc".to_vec());
        assert_eq!(pages.check_root(&terms), Ok(()));
        let before = pages.snapshot(&path("/")).unwrap().unwrap();
        pages.publish(path("/"), html(), b"new".to_vec());
        // PG-07: a pinned snapshot keeps its old bytes while a new read sees the new ones.
        assert_eq!(before.content.as_slice(), b"abc");
        assert_eq!(
            pages
                .snapshot(&path("/"))
                .unwrap()
                .unwrap()
                .content
                .as_slice(),
            b"new"
        );
        pages.remove(&path("/"));
        assert_eq!(pages.snapshot(&path("/")).unwrap(), None);
        assert_eq!(pages.snapshot(&path("/missing")).unwrap(), None);
    }

    // PG-06: the directory source maps the root to index.html, nested paths to files and labels by extension; a missing file is simply absent.
    #[test]
    fn the_directory_source_maps_paths_to_files() {
        let root = dir("map");
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("index.html"), b"<h1>hi</h1>").unwrap();
        std::fs::write(root.join("a/style.css"), b"h1{}").unwrap();
        std::fs::write(root.join("blob.bin"), b"\0\x01").unwrap();
        let pages = DirectoryPages::new(root, 100);
        let home = pages.snapshot(&CanonicalPath::root()).unwrap().unwrap();
        assert_eq!(
            (home.media_type.as_str(), home.content.as_slice()),
            ("text/html", &b"<h1>hi</h1>"[..])
        );
        assert_eq!(
            pages
                .snapshot(&path("/a/style.css"))
                .unwrap()
                .unwrap()
                .media_type
                .as_str(),
            "text/css"
        );
        assert_eq!(
            pages
                .snapshot(&path("/blob.bin"))
                .unwrap()
                .unwrap()
                .media_type
                .as_str(),
            "application/octet-stream"
        );
        assert_eq!(pages.snapshot(&path("/missing")).unwrap(), None);
        // A directory is not a resource.
        assert_eq!(pages.snapshot(&path("/a")).unwrap(), None);
    }

    // PG-06: a symbolic link, whether a file or a directory, never reaches outside the root; a file above the bound is a limit failure.
    #[test]
    fn symbolic_links_and_oversized_files_are_refused() {
        let root = dir("escape");
        let outside = dir("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("dir")).unwrap();
        std::fs::write(root.join("big.txt"), vec![b'x'; 101]).unwrap();
        let pages = DirectoryPages::new(root, 100);
        assert_eq!(pages.snapshot(&path("/link.txt")).unwrap(), None);
        assert_eq!(pages.snapshot(&path("/dir/secret.txt")).unwrap(), None);
        assert_eq!(pages.snapshot(&path("/big.txt")), Err(Error::Limit));
    }
}
