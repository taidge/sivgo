use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

/// Write a replacement beside the destination and publish it only after the
/// writer and the file sync succeed. Existing symlinks are followed.
pub fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let target = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => Some(fs::canonicalize(path)?),
        Ok(_) => None,
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let path = target.as_deref().unwrap_or(path);
    let permissions = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            // Keep the old write-access check even when directory permissions
            // would allow replacing a file the user cannot write directly.
            OpenOptions::new().write(true).open(path)?;
            Some(metadata.permissions())
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination is not a regular file",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let parent = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::Builder::new()
        .prefix(".sivgo-save-")
        .tempfile_in(parent)?;
    write(staged.as_file_mut())?;
    if let Some(permissions) = permissions {
        staged.as_file().set_permissions(permissions)?;
    }
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn failed_write_keeps_original_and_removes_partial_file() {
        let folder = tempfile::tempdir().unwrap();
        let drawing = folder.path().join("drawing.svg");
        fs::write(&drawing, b"original").unwrap();
        let failure = write_atomic(&drawing, |file| {
            file.write_all(b"partial replacement")?;
            Err(io::Error::other("simulated write failure"))
        });
        assert!(failure.is_err());
        assert_eq!(fs::read(&drawing).unwrap(), b"original");
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
        write_atomic(&drawing, |file| file.write_all(b"new version")).unwrap();
        assert_eq!(fs::read(&drawing).unwrap(), b"new version");
    }
}
