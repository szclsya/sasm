use super::{ExecutionError, PkgAction};
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn execute_pkg_actions(actions: &[PkgAction], root: &Path) -> Result<(), ExecutionError> {
    let mut to_install = vec!["--install".to_string()];
    let mut to_remove = vec!["--remove".to_string()];
    let mut to_purge = vec!["--purge".to_string()];
    let mut to_configure = vec!["--configure".to_string()];
    // Organize and download stuff
    for action in actions {
        match action {
            PkgAction::Install(_, url) | PkgAction::Upgrade(_, url) => {
                let path = download(url, root)?;
                to_install.push(path.to_str().unwrap().to_string());
            }
            PkgAction::Remove(names, purge) => {
                if *purge {
                    to_purge.append(&mut names.clone());
                } else {
                    to_remove.append(&mut names.clone());
                }
            }
            PkgAction::Reconfigure(name) => {
                to_configure.push(name.clone());
            }
        }
    }

    // Purge stuff
    dpkg_run(to_purge.as_slice(), root)?;
    // Remove stuff
    dpkg_run(to_remove.as_slice(), root)?;
    // Configure stuff
    dpkg_run(to_configure.as_slice(), root)?;
    // Install stuff
    dpkg_run(to_install.as_slice(), root)?;
    Ok(())
}

fn download(url: &str, root: &Path) -> Result<PathBuf, ExecutionError> {
    let client = Client::new();

    println!("Downloading {}", url);
    let mut response = client.get(url).send()?;
    if response.status() != 200 {
        return Err(ExecutionError::ResourceFetchError(format!(
            "cannot fetch {}",
            url
        )));
    }
    // Get filename
    let filename = response
        .url()
        .path_segments()
        .and_then(|segments| segments.last())
        .and_then(|name| if name.is_empty() { None } else { Some(name) })
        .ok_or_else(|| {
            ExecutionError::ResourceFetchError(format!("{} doesn't contain filename", url))
        })?;

    // Temp dir
    let download_path = root.join("var/cache/apm");
    if !download_path.is_dir() {
        fs::create_dir(&download_path)?;
    }
    let temp_file = &download_path.join(&filename);
    if !temp_file.is_file() {
        let mut dest = fs::File::create(temp_file)?;
        std::io::copy(&mut response, &mut dest)?;
    }

    Ok(temp_file.to_path_buf())
}

fn dpkg_run<T: AsRef<std::ffi::OsStr>>(args: &[T], root: &Path) -> Result<(), ExecutionError> {
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
        .map_err(|e| ExecutionError::InternalError(e.to_string()))?;
    if res.success() {
        Ok(())
    } else {
        match res.code() {
            Some(code) => Err(ExecutionError::DpkgError(code)),
            None => Err(ExecutionError::DpkgTerminated),
        }
    }
}
