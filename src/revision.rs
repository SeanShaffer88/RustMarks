//! Exact-byte optimistic concurrency checks for files opened by the editor.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct FileRevision {
    pub path: PathBuf,
    bytes: Vec<u8>,
    opened_path: PathBuf,
}
impl FileRevision {
    /// Capture the same bytes the parser will read, not a second metadata-only observation.
    pub fn read(path: &Path) -> Result<Self> {
        let canonical = std::fs::canonicalize(path)?;
        let file = std::fs::File::open(&canonical)?;
        if file.metadata()?.len() > MAX_DOCUMENT_BYTES as u64 {
            return Err("Bookmark file exceeds the 64 MiB import limit".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_DOCUMENT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err("Bookmark file exceeds the 64 MiB import limit".into());
        }
        Ok(Self {
            path: canonical,
            bytes,
            opened_path: std::path::absolute(path)?,
        })
    }
    pub fn text(&self) -> Result<&str> {
        Ok(std::str::from_utf8(&self.bytes)?)
    }
    /// Record our exact published bytes. A subsequent external change must not become our baseline.
    pub fn written(path: &Path, bytes: Vec<u8>) -> Self {
        Self {
            path: std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
            bytes,
            opened_path: std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
        }
    }
    pub fn refers_to(&self, path: &Path) -> bool {
        self.path == path
            || std::path::absolute(path).is_ok_and(|p| p == self.opened_path)
            || std::fs::canonicalize(path).is_ok_and(|p| p == self.path)
    }
    pub fn ensure_unchanged(&self) -> Result<()> {
        let current = Self::read(&self.opened_path).map_err(|e| {
            format!("Cannot verify the source file ({e}). Reopen it or save to a different path.")
        })?;
        if current.path != self.path || current.bytes != self.bytes {
            return Err("The source file changed outside RustMarks. Reopen it or save to a different path; the external changes have not been overwritten.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        directory: PathBuf,
        path: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(format!(
                "rustmarks-revision-{}-{now}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("bookmarks.json");
            std::fs::write(&path, b"original").unwrap();
            Self { directory, path }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
    #[test]
    fn unchanged_bytes_can_be_saved_but_same_length_edits_are_detected() {
        let fixture = Fixture::new();
        let revision = FileRevision::read(&fixture.path).unwrap();
        revision.ensure_unchanged().unwrap();
        assert_eq!(revision.text().unwrap(), "original");
        std::fs::write(&fixture.path, b"external").unwrap();
        assert!(revision.ensure_unchanged().is_err());
        assert_eq!(std::fs::read(&fixture.path).unwrap(), b"external");
    }
    #[test]
    fn deleted_sources_are_not_silently_recreated() {
        let fixture = Fixture::new();
        let revision = FileRevision::read(&fixture.path).unwrap();
        std::fs::remove_file(&fixture.path).unwrap();
        assert!(revision.refers_to(&fixture.path));
        assert!(revision.ensure_unchanged().is_err());
        assert!(!fixture.path.exists());
    }
    #[test]
    fn publication_keeps_our_bytes_as_baseline_not_a_later_disk_change() {
        let fixture = Fixture::new();
        std::fs::write(&fixture.path, b"published").unwrap();
        let revision = FileRevision::written(&fixture.path, b"published".to_vec());
        std::fs::write(&fixture.path, b"external").unwrap();
        assert!(revision.ensure_unchanged().is_err());
    }
    #[test]
    fn oversized_sources_are_rejected_before_allocation() {
        let fixture = Fixture::new();
        std::fs::File::create(&fixture.path)
            .unwrap()
            .set_len(MAX_DOCUMENT_BYTES as u64 + 1)
            .unwrap();
        assert!(FileRevision::read(&fixture.path)
            .unwrap_err()
            .to_string()
            .contains("64 MiB"));
    }
    #[test]
    fn source_aliases_match_the_same_file() {
        let fixture = Fixture::new();
        let revision = FileRevision::read(&fixture.path).unwrap();
        let alias = fixture
            .directory
            .join("..")
            .join(fixture.directory.file_name().unwrap())
            .join("bookmarks.json");
        assert!(revision.refers_to(&alias));
        assert!(!revision.refers_to(&fixture.directory.join("other.json")));
    }
    #[test]
    fn invalid_utf8_is_rejected_without_mutating_the_source() {
        let fixture = Fixture::new();
        std::fs::write(&fixture.path, [255, 254]).unwrap();
        let revision = FileRevision::read(&fixture.path).unwrap();
        assert!(revision.text().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn retargeted_symlinks_are_not_treated_as_an_unrelated_destination() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let alias = fixture.directory.join("alias.json");
        symlink(&fixture.path, &alias).unwrap();
        let revision = FileRevision::read(&alias).unwrap();
        let other = fixture.directory.join("other.json");
        std::fs::write(&other, b"original").unwrap();
        std::fs::remove_file(&alias).unwrap();
        symlink(&other, &alias).unwrap();
        assert!(revision.refers_to(&alias));
        assert!(revision.ensure_unchanged().is_err());
    }
}
