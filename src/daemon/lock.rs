//! Single-instance lock: flock + pidfile. The kernel releases the flock on
//! process death (including SIGKILL), so no stale-lock state is possible.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;

pub struct PidLock {
    file: File,
    pub path: PathBuf,
}

impl PidLock {
    /// Acquire the lock exclusively, writing our pid. Errors if a live
    /// process already holds it.
    pub fn acquire(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(pid) = peek(path) {
            anyhow::bail!("already running (pid {pid})");
        }
        // Truncating the stale pidfile early is safe: the live-pid pre-check
        // above already bailed, so no live daemon owns the file yet.
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        // The live-pid pre-check above already bailed; the blocking lock only
        // contends in the rare race where a daemon starts concurrently.
        if let Err(e) = file.lock_exclusive() {
            let pid = peek(path);
            anyhow::bail!("already running (pid {:?}): {e}", pid);
        }
        writeln!(file, "{}", std::process::id())?;
        file.flush()?;
        Ok(PidLock {
            file,
            path: path.to_path_buf(),
        })
    }

    pub fn release(self) {
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// The pid recorded in the lock file, if parseable.
pub fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// The live pid if the lock is held by a running process.
pub fn peek(path: &Path) -> Option<u32> {
    let pid = read_pid(path)?;
    if is_alive(pid) { Some(pid) } else { None }
}

/// True if a process with this pid exists (EPERM also counts: it exists but
/// is owned by another user).
pub fn is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let r = unsafe { libc::kill(pid as i32, 0) };
    if r == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
