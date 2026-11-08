use crate::{models::BookmarkDocument, parser::Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use std::{
    io::{Read, Write},
    path::PathBuf,
};
#[derive(Clone)]
pub struct Snapshot {
    pub path: PathBuf,
    pub label: String,
}
pub struct History {
    pub directory: PathBuf,
}
impl Default for History {
    fn default() -> Self {
        Self {
            directory: crate::paths::data_dir().join("history"),
        }
    }
}
impl History {
    pub fn save(&self, doc: &BookmarkDocument, reason: &str) -> Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let name = format!(
            "{}-{}-{}.json.gz",
            chrono::Utc::now().format("%Y%m%d-%H%M%S%.9f"),
            reason,
            uuid::Uuid::new_v4()
        );
        let target = self.directory.join(name);
        doc.validate()?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&serde_json::to_vec(doc)?)?;
        crate::storage::atomic_write(&target, &encoder.finish()?)?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Snapshot>> {
        if !self.directory.exists() {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for entry in std::fs::read_dir(&self.directory)? {
            let path = entry?.path();
            if path.extension().is_some_and(|s| s == "gz") {
                out.push(Snapshot {
                    label: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    path,
                });
            }
        }
        out.sort_by(|a, b| b.label.cmp(&a.label));
        Ok(out)
    }
    /// Delete the oldest snapshots beyond `keep_last` (0 = keep all).
    /// Returns how many were removed. Snapshot names sort newest-first,
    /// so everything past the first `keep_last` entries goes.
    pub fn prune(&self, keep_last: usize) -> Result<usize> {
        if keep_last == 0 {
            return Ok(0);
        }
        let snapshots = self.list()?;
        let mut removed = 0;
        for snapshot in snapshots.iter().skip(keep_last) {
            std::fs::remove_file(&snapshot.path)?;
            removed += 1;
        }
        Ok(removed)
    }
    /// Total bytes the stored snapshots occupy.
    pub fn total_size(&self) -> u64 {
        self.list()
            .map(|snapshots| {
                snapshots
                    .iter()
                    .filter_map(|s| std::fs::metadata(&s.path).ok())
                    .map(|m| m.len())
                    .sum()
            })
            .unwrap_or(0)
    }
    pub fn restore(&self, snapshot: &Snapshot) -> Result<BookmarkDocument> {
        let decoder = GzDecoder::new(std::fs::File::open(&snapshot.path)?);
        let mut bytes = Vec::new();
        decoder.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Snapshot exceeds the 64 MiB restore limit".into());
        }
        let doc: BookmarkDocument = serde_json::from_slice(&bytes)?;
        doc.validate()?;
        Ok(doc)
    }
}
