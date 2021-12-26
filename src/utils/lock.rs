use crate::{debug, warn, LOCK_PATH};
use anyhow::{bail, Context, Result};
use nix::unistd::Uid;
use serde::{Deserialize, Serialize};
use std::{fs, io::prelude::*, path::Path, sync::atomic::Ordering};

/// Make sure only one instance of Omakase can run at one time

#[derive(Serialize, Deserialize)]
struct LockInfo {
    pid: u32,
}

pub fn ensure_unlocked(root: &Path) -> Result<()> {
    if let Some(pid) = check(root)? {
        bail!(
            "Another instance of Omakase is currently running at PID {}",
            pid
        );
    }

    Ok(())
}

pub fn check(root: &Path) -> Result<Option<u32>> {
    let lock_path = root.join(LOCK_PATH);
    if lock_path.is_file() {
        let lock_content =
            std::fs::read_to_string(lock_path).context("Failed to read lock file")?;
        let lock_info: LockInfo =
            toml::from_str(&lock_content).context("Failed to parse lock file")?;
        Ok(Some(lock_info.pid))
    } else {
        Ok(None)
    }
}

pub fn lock(root: &Path) -> Result<()> {
    // Make sure we are running as root
    if !Uid::effective().is_root() {
        bail!("You must be root to perform this operation");
    }

    let lock_path = root.join(LOCK_PATH);
    if lock_path.is_file() {
        bail!("Cannot lock because lock file already exists");
    }

    // Set global lock parameter
    crate::LOCKED.store(true, Ordering::Relaxed);

    // Set up SIGINT handler
    {
        let root = root.to_owned();
        ctrlc::set_handler(move || {
            if crate::DPKG_RUNNING.load(Ordering::Relaxed) {
                warn!("Cannot interrupt when dpkg is running");
            } else {
                // Thanks to stateless, we can just exit
                unlock(&root).expect("Failed to unlock");
                std::process::exit(2);
            }
        })
        .expect("Error setting SIGINT handler");
    }

    // Create directory if not created yet
    let prefix = lock_path.parent().unwrap();
    if !prefix.is_dir() {
        fs::create_dir_all(prefix).context("Failed to create dir for lock file")?;
    }
    let lock_info = LockInfo {
        pid: std::process::id(),
    };
    let lock_content = toml::to_string(&lock_info)?;
    let mut file = fs::File::create(&lock_path).context("Failed to create lock file")?;
    file.write(lock_content.as_bytes())
        .context("Failed to write lock content")?;
    Ok(())
}

pub fn unlock(root: &Path) -> Result<()> {
    let lock_path = root.join(LOCK_PATH);
    if lock_path.is_file() {
        fs::remove_file(&lock_path).context("Failed to delete lock file")?;
    } else {
        debug!("Attempt to unlock, but lock file doesn't exist");
    }
    Ok(())
}
