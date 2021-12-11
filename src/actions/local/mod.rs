mod deb;

use super::{execute, UserRequest};
use crate::{
    db::LocalDb,
    executor::MachineStatus,
    info,
    types::config::{Blueprints, Config, IgnoreRules, Opts},
    utils::downloader::Downloader,
    warn,
};
use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command, sync::atomic::Ordering};

pub async fn install_deb(
    p: &Path,
    local_db: &LocalDb,
    downloader: &Downloader,
    blueprint: &mut Blueprints,
    ignorerules: &mut IgnoreRules,
    opts: &Opts,
    config: &Config,
) -> Result<()> {
    warn!("Please make sure this deb is for AOSC OS");
    if !dialoguer::Confirm::new()
        .with_prompt(format!("{}{}", crate::cli::gen_prefix(""), "Proceed?"))
        .interact()?
    {
        bail!("User cancelled operation");
    }

    // Read control of deb
    info!("Reading deb file...");
    let meta = deb::read_control_from_deb(p)?;
    // Install dependencies
    info!("Installing dependencies...");
    for dependency in meta.depends {
        let ver_req = if dependency.1.is_arbitary() {
            None
        } else {
            Some(dependency.1)
        };
        blueprint.add(&dependency.0, Some(&meta.name), ver_req).ok();
    }
    execute(
        local_db,
        downloader,
        blueprint,
        ignorerules,
        opts,
        config,
        UserRequest::Upgrade,
    )
    .await?;

    // Check conflicts
    info!("Checking conflicts...");
    let ms = MachineStatus::new(&opts.root)?;
    for conflict in &meta.conflicts {
        if let Some(pkgstatus) = ms.pkgs.get(&conflict.0) {
            if conflict.1.within(&pkgstatus.version) {
                bail!("This deb conflicts with {} on local machine", conflict.0);
            }
        }
    }
    for b in &meta.breaks {
        if let Some(pkgstatus) = ms.pkgs.get(&b.0) {
            if b.1.within(&pkgstatus.version) {
                bail!("This deb breaks {} on local machine", b.0);
            }
        }
    }

    // Run installation
    let args = vec!["--install".to_string(), p.display().to_string()];
    dpkg_run(&args, &opts.root)?;
    // Add this package to user IgnoreRules table
    ignorerules.add(meta.name).ok();

    Ok(())
}

pub async fn remove_deb(
    pkgname: &str,
    local_db: &LocalDb,
    downloader: &Downloader,
    blueprint: &mut Blueprints,
    ignorerules: &mut IgnoreRules,
    opts: &Opts,
    config: &Config,
) -> Result<()> {
    let ms = MachineStatus::new(&opts.root)?;
    if !ms.pkgs.contains_key(pkgname) {
        bail!("No such package on local system");
    }

    // Remove IgnoreRule
    ignorerules.remove(pkgname).ok();
    // Remove dependencies
    blueprint.remove_affiliated(pkgname);
    execute(
        local_db,
        downloader,
        blueprint,
        ignorerules,
        opts,
        config,
        UserRequest::Upgrade,
    )
    .await?;

    Ok(())
}

fn dpkg_run<T: AsRef<std::ffi::OsStr>>(args: &[T], root: &Path) -> Result<()> {
    let mut cmd = Command::new("dpkg");
    // Add root position
    cmd.arg("--root");
    cmd.arg(root.as_os_str());
    // If no stuff is specified, success automatically
    if args.len() <= 1 {
        return Ok(());
    }
    // Add rest of the arguments
    cmd.args(args);

    // Tell the signal handler we are going to run dpkg
    crate::DPKG_RUNNING.store(true, Ordering::Relaxed);
    // Run it!
    let res = cmd.status().context("dpkg command execution failed")?;
    if !res.success() {
        match res.code() {
            Some(code) => bail!("dpkg exited with non-zero return code {}", code),
            None => bail!("dpkg exited by signal"),
        }
    }

    // We are done with dpkg
    crate::DPKG_RUNNING.store(false, Ordering::Relaxed);
    Ok(())
}
