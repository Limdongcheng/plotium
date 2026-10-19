//! Captured save identity and explicit, revocable external-write authorization.
use super::{ProjectError, ProjectSaveRequest, SaveOutcome, persistence};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::SystemTime,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SavePhase {
    NotInstalled,
    InstalledUnconfirmed,
    Committed,
}

impl ProjectError {
    pub fn save_phase(&self) -> SavePhase {
        match self {
            Self::InstalledUnconfirmed(_) => SavePhase::InstalledUnconfirmed,
            _ => SavePhase::NotInstalled,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SaveSnapshotIdentity {
    pub project_context: crate::state::ProjectContextId,
    pub document_revision: u64,
    pub edit_generation: u64,
}

impl ProjectSaveRequest {
    pub fn progress(&self) -> SaveProgress {
        self.progress.clone()
    }
    pub fn identity(&self) -> &SaveSnapshotIdentity {
        &self.identity
    }
    pub fn persisted_revision(&self) -> &str {
        &self.revision
    }
    pub fn authorize(&mut self, authorization: SaveAuthorization) {
        self.authorization = Some(authorization);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetStamp {
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    inode: (u64, u64),
}
fn stamp(path: &Path) -> Result<Option<TargetStamp>, ProjectError> {
    let m = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !m.is_file() || m.file_type().is_symlink() {
        return Err(ProjectError::Invalid(
            "Choose a regular, non-symlink .plotium target".into(),
        ));
    }
    Ok(Some(TargetStamp {
        len: m.len(),
        modified: m.modified()?,
        #[cfg(unix)]
        inode: {
            use std::os::unix::fs::MetadataExt;
            (m.dev(), m.ino())
        },
    }))
}

/// Target observed in preflight and shown verbatim in the desktop confirmation.
#[derive(Clone, Debug)]
pub struct SaveTarget {
    pub path: PathBuf,
    stamp: Option<TargetStamp>,
}
impl SaveTarget {
    pub fn inspect(path: &Path) -> Result<Self, ProjectError> {
        if !path.is_absolute() || path.extension().is_none_or(|e| e != "plotium") {
            return Err(ProjectError::Invalid(
                "Supply an absolute .plotium path".into(),
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| ProjectError::Invalid("Target needs a parent directory".into()))?
            .canonicalize()?;
        let name = path
            .file_name()
            .ok_or_else(|| ProjectError::Invalid("Target needs a filename".into()))?;
        let path = parent.join(name);
        Ok(Self {
            stamp: stamp(&path)?,
            path,
        })
    }
    pub fn exists(&self) -> bool {
        self.stamp.is_some()
    }
    pub fn verify(&self) -> Result<(), ProjectError> {
        if stamp(&self.path)? != self.stamp {
            return Err(ProjectError::Invalid(
                "Save target changed; inspect it and request fresh authorization".into(),
            ));
        }
        Ok(())
    }
}

/// The atomic permission transition admits installation once, without blocking
/// the desktop on filesystem I/O. Revocation before admission prevents installation;
/// after admission the original snapshot/path may complete and must be checked.
#[derive(Clone, Debug)]
pub struct SaveAuthorization {
    valid: Arc<AtomicU8>,
    target: SaveTarget,
}
impl SaveAuthorization {
    pub fn pending(target: SaveTarget) -> Self {
        Self {
            valid: Arc::new(AtomicU8::new(0)),
            target,
        }
    }
    pub fn grant(&self, overwrite_confirmed: bool) -> Result<(), ProjectError> {
        self.target.verify()?;
        if self.target.exists() && !overwrite_confirmed {
            return Err(ProjectError::Invalid(
                "Confirm replacement of the existing target".into(),
            ));
        }
        self.valid
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                ProjectError::Invalid("Save authorization was revoked or already used".into())
            })?;
        Ok(())
    }
    pub fn revoke(&self) {
        self.valid.store(3, Ordering::Release);
    }
    pub fn is_revoked(&self) -> bool {
        self.valid.load(Ordering::Acquire) == 3
    }
    pub fn is_valid(&self) -> bool {
        matches!(self.valid.load(Ordering::Acquire), 1 | 2)
    }
    pub(super) fn commit(
        &self,
        temp: &Path,
        path: &Path,
        backups: usize,
        progress: &SaveProgress,
    ) -> Result<Option<String>, ProjectError> {
        if path != self.target.path
            || self
                .valid
                .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err(ProjectError::Invalid(
                "Save authorization was revoked or already used".into(),
            ));
        }
        self.target.verify()?;
        // Admission is one-use permission, not a target lock. The existing-file
        // path rechecks this stamp after backup preparation, next to replacement.
        if self.target.exists() {
            persistence::commit_project_file_with_progress(
                temp,
                path,
                backups,
                progress,
                Some(&self.target),
            )
        } else {
            persistence::commit_new_project_file(temp, path, progress)
        }
    }
}

pub fn save_result_phase(result: &Result<SaveOutcome, ProjectError>) -> SavePhase {
    match result {
        Ok(_) => SavePhase::Committed,
        Err(e) => e.save_phase(),
    }
}

/// Stage witness survives worker failure; only the native installation boundary
/// may advance it. It never infers target state from an error message.
#[derive(Clone, Default)]
pub struct SaveProgress(Arc<std::sync::atomic::AtomicU8>);
impl SaveProgress {
    pub fn phase(&self) -> SavePhase {
        match self.0.load(std::sync::atomic::Ordering::Acquire) {
            0 => SavePhase::NotInstalled,
            1 => SavePhase::InstalledUnconfirmed,
            _ => SavePhase::Committed,
        }
    }
    pub(super) fn installed(&self) {
        self.0.store(1, std::sync::atomic::Ordering::Release);
    }
    pub(super) fn committed(&self) {
        self.0.store(2, std::sync::atomic::Ordering::Release);
    }
}
pub(super) struct SaveCommitOptions<'a> {
    pub backups: usize,
    pub authorization: Option<&'a SaveAuthorization>,
    pub progress: &'a SaveProgress,
}
