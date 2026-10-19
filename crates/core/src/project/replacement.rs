//! Windows replacement failures can have filesystem side effects.
use std::{io, path::Path};

// Only ReplaceFileW's documented partial failures carry this marker. Ordinary
// I/O/authorization failures must still remove the private serialized candidate.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct PartialReplacement {
    message: String,
    restore_previous_backup: bool,
}

pub(in crate::project) fn requires_save_recovery(error: &super::ProjectError) -> bool {
    matches!(error, super::ProjectError::Io(error)
        if error.get_ref().is_some_and(|inner| inner.is::<PartialReplacement>()))
}

pub(super) fn finish_partial_rollback(
    error: &mut io::Error,
    target: &Path,
    staged: Option<&Path>,
) -> bool {
    let Some(partial) = error
        .get_mut()
        .and_then(|inner| inner.downcast_mut::<PartialReplacement>())
    else {
        return false;
    };
    if let Some(staged) = staged {
        if partial.restore_previous_backup {
            let primary = super::backup_path(target, 0);
            if let Err(restore_error) = restore_without_replace(staged, &primary) {
                partial.message.push_str(&format!(
                    "; previous backup retained at {}; could not finish restoring {}: {restore_error}",
                    staged.display(), primary.display()
                ));
            }
        } else {
            // The primary backup can now be the original target. Never delete it
            // to put an older backup back in its customary slot.
            partial.message.push_str(&format!(
                "; previous backup retained at {}",
                staged.display()
            ));
        }
    }
    true
}

// Keep the recovery source until the destination is linked without overwrite
// and synced. Failure leaves all remaining recovery names for manual inspection.
fn restore_without_replace(source: &Path, target: &Path) -> io::Result<()> {
    std::fs::hard_link(source, target)?;
    super::sync_file(target)?;
    super::sync_parent(target)?;
    std::fs::remove_file(source)
}

#[cfg(any(windows, test))]
fn windows_replace_error(
    error: io::Error,
    temp: &Path,
    target: &Path,
    backup: Option<&Path>,
) -> io::Error {
    let code = error.raw_os_error();
    // 1176 with a backup, 1175 and other errors leave both original names intact.
    // 1176 without a backup removes the old target; 1177 moves it elsewhere.
    // https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew
    if !matches!(code, Some(1177)) && !(code == Some(1176) && backup.is_none()) {
        return error;
    }
    let mut message = format!(
        "Windows ReplaceFileW failed: {error}; snapshot not installed at {}; serialized snapshot retained at {}",
        target.display(),
        temp.display()
    );
    let mut restore_previous_backup = false;
    if code == Some(1177) {
        if let Some(backup) = backup {
            let restored = restore_without_replace(backup, target);
            match restored {
                Ok(()) => {
                    restore_previous_backup = true;
                    message.push_str(&format!("; original target restored at {}", target.display()));
                }
                Err(restore_error) => message.push_str(&format!(
                    "; could not complete original-target restoration: {restore_error}; inspect target {} and original recovery file {}",
                    target.display(), backup.display()
                )),
            }
        } else {
            message.push_str("; Windows moved the original file to an unspecified name; its recovery path was not returned");
        }
    } else {
        message.push_str("; Windows removed the original target without creating a backup");
    }
    io::Error::new(
        error.kind(),
        PartialReplacement {
            message,
            restore_previous_backup,
        },
    )
}

#[cfg(windows)]
pub(super) fn platform_replace_checked(
    temp: &Path,
    target: &Path,
    backup: Option<&Path>,
    verify: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_WRITE_THROUGH, MoveFileExW, ReplaceFileW,
    };

    let target_exists = target.exists();
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    };
    let temp_wide = wide(temp);
    let target_wide = wide(target);
    let backup_wide = backup.map(wide);
    verify()?;
    // SAFETY: all pointers refer to NUL-terminated buffers that remain alive for
    // the call. ReplaceFileW may partially move/delete names on failure; the
    // error handler below preserves recovery files before ordinary rollback.
    let ok = unsafe {
        if target_exists {
            ReplaceFileW(
                target_wide.as_ptr(),
                temp_wide.as_ptr(),
                backup_wide
                    .as_ref()
                    .map_or(std::ptr::null(), |p| p.as_ptr()),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } else {
            MoveFileExW(
                temp_wide.as_ptr(),
                target_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if ok == 0 {
        let error = io::Error::last_os_error();
        Err(if target_exists {
            windows_replace_error(error, temp, target, backup)
        } else {
            error
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
type TestReplacer = Box<dyn FnOnce(&Path, &Path, Option<&Path>) -> io::Result<()>>;
#[cfg(test)]
thread_local! {
    pub(super) static REPLACE_TEST: std::cell::RefCell<Option<TestReplacer>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
#[path = "replacement_tests.rs"]
mod tests;
