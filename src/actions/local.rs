use crate::{
    cli, debug,
    executor::{MachineStatus, PkgState},
    info,
    pool::source::local,
};

use anyhow::{bail, Result};
use console::style;
use dialoguer::Confirm;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

pub fn add(paths: &[PathBuf], root: &Path) -> Result<Vec<String>> {
    let local_repo_root = root.join(crate::LOCAL_REPO_PATH);

    let mut jobs = Vec::new();
    for path in paths {
        // Try to load deb info
        let pkgmeta = crate::pool::source::local::read_control_from_deb(path)?;
        info!(
            "Loading {}({}) into local package repository...",
            style(&pkgmeta.name).bold(),
            pkgmeta.version
        );
        if !Confirm::new()
            .with_prompt(format!("{}{}", cli::gen_prefix(""), "Confirm?"))
            .interact()?
        {
            bail!("User cancelled operation.");
        }

        let filename = match path.file_name() {
            Some(f) => f,
            None => bail!("Invalid deb file {} !", path.display()),
        };

        // Prepare job
        let new_path = local_repo_root.join(filename);
        jobs.push((path, new_path, pkgmeta));
    }

    // User has confirmed all packages.
    let mut res = Vec::new();
    if !local_repo_root.is_dir() {
        fs::create_dir_all(&local_repo_root)?;
    }
    for (old_path, new_path, pkg) in jobs {
        // Copy package to local repo
        std::fs::copy(old_path, new_path)?;
        // Add pkgname to res
        res.push(pkg.name);
    }

    Ok(res)
}

pub fn clean(ms: &MachineStatus, root: &Path) -> Result<()> {
    let local_repo_root = root.join(crate::LOCAL_REPO_PATH);
    if !local_repo_root.is_dir() {
        // Nothing to clean
        return Ok(());
    }

    for entry in fs::read_dir(&local_repo_root)? {
        let entry = entry?;
        let path = entry.path();
        debug!("Inspecting local deb {} ...", path.display());
        if !path.is_file() || path.extension() != Some(OsStr::new("deb")) {
            continue;
        }
        // Read meta
        let pkgmeta = local::read_control_from_deb(&path)?;
        if let Some(pkgstate) = ms.pkgs.get(&pkgmeta.name) {
            if pkgstate.state != PkgState::ConfigFiles && pkgstate.state != PkgState::NotInstalled {
                // This package is needed. Move on.
                continue;
            }
        }
        // Not contained in current machine or not installed, remove it.
        debug!("Removing {} ...", style(path.display()).bold());
        fs::remove_file(&path)?;
    }

    Ok(())
}
