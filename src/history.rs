use crate::{models::BookmarkDocument, parser::Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use std::{io::Write, path::PathBuf};
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
        let base = std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("XDG_DATA_HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".local/share")
            });
        Self {
            directory: base.join("RustMarks/history"),
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
        let temp = target.with_extension("tmp");
        let mut encoder = GzEncoder::new(std::fs::File::create(&temp)?, Compression::default());
        encoder.write_all(&serde_json::to_vec(doc)?)?;
        encoder.finish()?.sync_all()?;
        std::fs::rename(temp, target)?;
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
    pub fn restore(&self, snapshot: &Snapshot) -> Result<BookmarkDocument> {
        Ok(serde_json::from_reader(GzDecoder::new(
            std::fs::File::open(&snapshot.path)?,
        ))?)
    }
}
