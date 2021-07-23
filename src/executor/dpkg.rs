use super::download::Downloader;
use crate::types::PkgActions;

use anyhow::{Result, Context, bail};
use std::path::Path;
use std::process::Command;

pub async fn execute_pkg_actions(
    mut actions: PkgActions,
    root: &Path,
    downloader: &Downloader,
) -> Result<()> {
    // Download packages
    let download_info: Vec<(String, Option<String>, Option<u64>)> = actions
        .install
        .iter()
        .map(|x| (x.1.clone(), None, Some(x.2)))
        .collect();
    let download_res = downloader
        .fetch(download_info, &root.join("var/cache/apm/pkgs"))
        .await
        .context("Failed to fetch packages from repository")?;

    let mut deb_paths: Vec<String> = actions
        .install
        .iter()
        .map(|x| {
            download_res
                .get(&x.1)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect();

    // Purge stuff
    if !actions.purge.is_empty() {
        let mut cmd = vec!["--purge".to_string()];
        cmd.append(&mut actions.purge);
        dpkg_run(&cmd, root).context("Purge packages failed")?;
    }
    // Remove stuff
    if !actions.remove.is_empty() {
        let mut cmd = vec!["--remove".to_string()];
        cmd.append(&mut actions.remove);
        dpkg_run(&cmd, root).context("Remove packages failed")?;
    }
    // Configure stuff
    if !actions.configure.is_empty() {
        let mut cmd = vec!["--configure".to_string()];
        cmd.append(&mut actions.configure);
        dpkg_run(&cmd, root).context("Configure packages failed")?;
    }
    // Install stuff
    if !deb_paths.is_empty() {
        let mut cmd = vec!["--install".to_string()];
        cmd.append(&mut deb_paths);
        dpkg_run(&cmd, root).context("Install packages failed")?;
    }

    Ok(())
}

fn dpkg_run<T: AsRef<std::ffi::OsStr>>(args: &[T], root: &Path) -> Result<()> {
    let mut cmd = Command::new("dpkg");
    // Add root position
    cmd.arg("--root");
    cmd.arg(root.as_os_str());
    // Force all!
    cmd.arg("--force-all");
    // If no stuff is specified, success automatically
    if args.len() <= 1 {
        return Ok(());
    }
    // Add rest of the arguments
    cmd.args(args);

    // Run it!
    let res = cmd
        .status()
        .context("dpkg command execution failed")?;
    if res.success() {
        Ok(())
    } else {
        match res.code() {
            Some(code) => bail!("dpkg exited with non-zero return code {}", code),
            None => bail!("dpkg exited by signal"),
        }
    }
}