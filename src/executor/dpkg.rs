use super::{ExecutionError, PkgAction};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

pub fn execute_pkg_actions(actions: &[PkgAction], root: &Path) -> Result<(), ExecutionError> {
    let mut pkgname_to_path = HashMap::new();
    // First, download all the archives we will need
    for action in actions {
        match action {
            PkgAction::Install(name, url) | PkgAction::Upgrade(name, url) => {
                let path = download(url, root)?;
                pkgname_to_path.insert(name, path);
            }
            _ => (),
        }
    }
    for action in actions {
        match action {
            PkgAction::Install(name, _) | PkgAction::Upgrade(name, _) => {
                // Run install
                let path = pkgname_to_path.get(name).ok_or_else(|| {
                    ExecutionError::InteranlError(format!(
                        "Package archive for {} not found after download",
                        name
                    ))
                })?;
                dpkg_run(&["--install", path.to_str().unwrap()], root)?;
            },
            PkgAction::Remove(names, purge) => {
                // Prepare command
                let mut command = names.clone();
                if *purge {
                    command.insert(0, "--purge".to_string());
                } else {
                    command.insert(0, "--remove".to_string());
                }
                // Run it!
                dpkg_run(&command.as_slice(), root)?;
            }
            PkgAction::Reconfigure(name) => {
                dpkg_run(&["--configure", name], root)?;
            }
        }
    }
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
    // Add rest of the arguments
    cmd.args(args);

    // Run it!
    let res = cmd
        .status()
        .map_err(|e| ExecutionError::InteranlError(e.to_string()))?;
    if res.success() {
        Ok(())
    } else {
        match res.code() {
            Some(code) => Err(ExecutionError::DpkgError(code)),
            None => Err(ExecutionError::DpkgTerminated),
        }
    }
}
