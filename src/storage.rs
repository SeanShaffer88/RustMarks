use crate::parser::Result;
use std::{io::Write, path::Path};
/// Publish only complete, flushed files. Never remove the existing target first.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_file_name(format!(".rustmarks-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
