use std::{fs, io, path::Path};

#[derive(Debug, PartialEq, Eq)]
pub struct FileIdentity(PlatformIdentity);

#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
struct PlatformIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
struct PlatformIdentity(same_file::Handle);

impl FileIdentity {
    pub fn from_path(path: &Path, metadata: &fs::Metadata) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let _ = path;
            Ok(Self(PlatformIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            }))
        }
        #[cfg(windows)]
        {
            let _ = metadata;
            same_file::Handle::from_path(path)
                .map(PlatformIdentity)
                .map(Self)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::FileIdentity;

    #[test]
    fn identity_survives_append_and_changes_after_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.jsonl");
        std::fs::write(&path, "first\n").unwrap();
        let initial_metadata = std::fs::metadata(&path).unwrap();
        let initial = FileIdentity::from_path(&path, &initial_metadata).unwrap();

        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"second\n")
            .unwrap();
        let appended_metadata = std::fs::metadata(&path).unwrap();
        let appended = FileIdentity::from_path(&path, &appended_metadata).unwrap();
        assert_eq!(initial, appended);

        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "replacement\n").unwrap();
        let replacement_metadata = std::fs::metadata(&path).unwrap();
        let replacement = FileIdentity::from_path(&path, &replacement_metadata).unwrap();
        assert_ne!(initial, replacement);
    }
}
