use crate::{
    info,
    types::{PkgActions, PkgSource},
    utils::downloader::{Compression, DownloadJob, Downloader},
};

use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command, sync::atomic::Ordering};

pub async fn execute_pkg_actions(
    mut actions: PkgActions<'_>,
    root: &Path,
    downloader: &Downloader,
    unsafe_io: bool,
) -> Result<()> {
    // Download packages
    let download_jobs = get_download_jobs(&actions);
    info!("Fetching requested packages...");
    let download_res = downloader
        .fetch(download_jobs, &root.join(crate::PKG_CACHE_PATH), true)
        .await
        .context("Failed to fetch requested packages from repository.")?;

    let mut install_deb_paths: Vec<String> = actions
        .install
        .iter()
        .map(|(install, _)| match &install.source {
            PkgSource::Http((url, _, _)) => download_res.get(url).unwrap(),
            PkgSource::Local(p) => p,
        })
        .map(|p| p.to_str().unwrap().to_owned())
        .collect();

    let mut unpack_deb_paths: Vec<String> = actions
        .unpack
        .iter()
        .map(|(unpack, _)| match &unpack.source {
            PkgSource::Http((url, _, _)) => download_res.get(url).unwrap(),
            PkgSource::Local(p) => p,
        })
        .map(|p| p.to_str().unwrap().to_owned())
        .collect();

    info!("Processing package changes...");
    // Purge stuff
    if !actions.purge.is_empty() {
        let mut cmd = vec!["--purge".to_string()];
        let mut pkgnames: Vec<String> =
            actions.purge.into_iter().map(|(name, _, _)| name).collect();
        cmd.append(&mut pkgnames);
        dpkg_run(&cmd, root, unsafe_io).context("Failed to purge package configuration(s).")?;
    }
    // Remove stuff
    if !actions.remove.is_empty() {
        let mut cmd = vec!["--remove".to_string()];
        let mut pkgnames: Vec<String> = actions
            .remove
            .into_iter()
            .map(|(name, _, _)| name)
            .collect();
        cmd.append(&mut pkgnames);
        dpkg_run(&cmd, root, unsafe_io).context("Failed to remove package(s).")?;
    }
    // Configure stuff
    if !actions.configure.is_empty() {
        let mut cmd = vec!["--configure".to_string()];
        cmd.append(&mut actions.configure);
        dpkg_run(&cmd, root, unsafe_io).context("Failed to configure package(s).")?;
    }
    // Install stuff
    if !install_deb_paths.is_empty() {
        let mut cmd = vec!["--install".to_string()];
        cmd.append(&mut install_deb_paths);
        dpkg_run(&cmd, root, unsafe_io).context("Failed to install package(s).")?;
    }
    // Unpack stuff
    if !unpack_deb_paths.is_empty() {
        let mut cmd = vec!["--unpack".to_string()];
        cmd.append(&mut unpack_deb_paths);
        dpkg_run(&cmd, root, unsafe_io).context("Failed to unpack package(s).")?;
    }

    Ok(())
}

fn dpkg_run<T: AsRef<std::ffi::OsStr>>(args: &[T], root: &Path, unsafe_io: bool) -> Result<()> {
    let mut cmd = Command::new("dpkg");
    if unsafe_io {
        cmd.arg("--force-unsafe-io");
    }
    // Add root position
    cmd.arg("--root");
    cmd.arg(root.as_os_str());
    // Ignore dependency/break checks and essential. These will be guaranteed by Omakase
    cmd.args(&[
        "--force-downgrade",
        "--force-breaks",
        "--force-conflicts",
        "--force-depends",
        "--force-remove-essential",
    ]);
    // If no stuff is specified, success automatically
    if args.len() <= 1 {
        return Ok(());
    }
    // Add rest of the arguments
    cmd.args(args);

    // Tell the signal handler we are going to run dpkg
    crate::DPKG_RUNNING.store(true, Ordering::Relaxed);
    // Run it!
    let res = cmd.status().context("Failed to execute dpkg command(s).")?;
    if !res.success() {
        match res.code() {
            Some(code) => bail!("dpkg exited with non-zero return code: {}.", code),
            None => bail!("dpkg process was terminated by signal."),
        }
    }

    // We are done with dpkg
    crate::DPKG_RUNNING.store(false, Ordering::Relaxed);
    Ok(())
}

fn get_download_jobs(actions: &PkgActions) -> Vec<DownloadJob> {
    let mut res = Vec::new();
    for i in &actions.install {
        if let PkgSource::Http((url, size, checksum)) = &i.0.source {
            let job = DownloadJob {
                url: url.clone(),
                description: None,
                filename: None,
                size: Some(*size),
                compression: Compression::None(Some(checksum.clone())),
            };
            res.push(job);
        }
    }

    for i in &actions.unpack {
        if let PkgSource::Http((url, size, checksum)) = &i.0.source {
            let job = DownloadJob {
                url: url.clone(),
                description: None,
                filename: None,
                size: Some(*size),
                compression: Compression::None(Some(checksum.clone())),
            };
            res.push(job);
        }
    }
    res
}
