//! Exact output targets and one-use write admission. Never truncate a caller path.
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::SystemTime,
};
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    inode: (u64, u64),
}
fn stamp(path: &Path) -> Result<Option<Stamp>, String> {
    let m = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !m.is_file() || m.file_type().is_symlink() {
        return Err("Choose a regular non-symlink output file".into());
    }
    Ok(Some(Stamp {
        len: m.len(),
        modified: m.modified().map_err(|e| e.to_string())?,
        #[cfg(unix)]
        inode: {
            use std::os::unix::fs::MetadataExt;
            (m.dev(), m.ino())
        },
    }))
}
#[derive(Clone, Debug)]
pub struct ExportTarget {
    pub path: PathBuf,
    observed: Option<Stamp>,
}
impl ExportTarget {
    pub fn inspect(path: &Path, extension: &str) -> Result<Self, String> {
        if !path.is_absolute() || path.extension().is_none_or(|e| e != extension) {
            return Err(format!("Supply an absolute .{extension} path"));
        }
        let parent = path
            .parent()
            .ok_or("Output needs a parent directory")?
            .canonicalize()
            .map_err(|e| format!("Output directory unavailable: {e}"))?;
        if !parent.is_dir() {
            return Err("Output parent is not a directory".into());
        }
        let path = parent.join(path.file_name().ok_or("Output needs a filename")?);
        Ok(Self {
            observed: stamp(&path)?,
            path,
        })
    }
    pub fn exists(&self) -> bool {
        self.observed.is_some()
    }
    pub fn verify(&self) -> Result<(), String> {
        if stamp(&self.path)? != self.observed {
            Err("Output changed; inspect it and request fresh authorization".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn install_export(
        &self,
        bytes: &[u8],
        authorization: &ExportAuthorization,
    ) -> Result<(), String> {
        // Unique, exclusively owned staging cannot damage fixed sibling names or links.
        let mut temp = tempfile::Builder::new()
            .prefix(".plotium-export-")
            .tempfile_in(self.path.parent().ok_or("Output parent unavailable")?)
            .map_err(|e| e.to_string())?;
        temp.write_all(bytes)
            .and_then(|()| temp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        self.verify()?;
        if !authorization.admit() {
            return Err("Export authorization revoked before installation".into());
        }
        // This admission is permission, not a cross-process target lock. Existing-file
        // metadata is checked immediately before replacement; no concurrent-writer CAS.
        let installed = if self.exists() {
            temp.persist(&self.path)
                .map(|_| ())
                .map_err(|e| e.error.to_string())
        } else {
            temp.persist_noclobber(&self.path)
                .map(|_| ())
                .map_err(|e| e.error.to_string())
        };
        authorization.release();
        installed?;
        Ok(())
    }
}
#[derive(Clone, Debug, Default)]
pub struct ExportAuthorization(Arc<AtomicU8>);
impl ExportAuthorization {
    pub(crate) fn grant(&self) -> bool {
        self.0
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn admit(&self) -> bool {
        self.0
            .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn release(&self) {
        // Revocation wins over releasing a completed installation.
        match self
            .0
            .compare_exchange(2, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(3) => {}
            Err(_) => debug_assert!(false, "installation owns admission"),
        }
    }

    pub fn revoke(&self) {
        self.0.store(3, Ordering::Release);
    }
    pub fn is_revoked(&self) -> bool {
        self.0.load(Ordering::Acquire) == 3
    }
}
