use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Write `bytes` to `path` so that the destination is either untouched or
/// complete: the data goes to a sibling temp file, is flushed to disk, and is
/// then renamed over `path`. A crash or full disk half-way through never
/// leaves a truncated file where the user expects a backup.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = temp_sibling(path);
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_content_and_removes_temp_file() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("backup.json");

        write_atomically(&target, b"{\"ok\":true}").unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"{\"ok\":true}");
        assert!(!temp_sibling(&target).exists());
    }

    #[test]
    fn replaces_existing_file() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("backup.json");
        std::fs::write(&target, b"old").unwrap();

        write_atomically(&target, b"new").unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new");
    }

    #[test]
    fn leaves_target_untouched_when_directory_is_missing() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("missing").join("backup.json");

        let err = write_atomically(&target, b"x").unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(!target.exists());
        assert!(!temp_sibling(&target).exists());
    }
}
