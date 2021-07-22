use super::{download::Downloader, ExecutionError};
use crate::types::PkgActions;
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub async fn execute_pkg_actions(
    mut actions: PkgActions,
    root: &Path,
    downloader: &Downloader,
) -> Result<(), ExecutionError> {
    // Download packages
    let download_info: Vec<(String, String, Option<u64>)> = actions
        .install
        .iter()
        .map(|x| (x.0.clone(), x.1.clone(), Some(x.2)))
        .collect();
    let download_res = downloader
        .fetch(download_info, &root.join("var/cache/apm"))
        .await
        .map_err(|e| ExecutionError::ResourceFetchError(e.to_string()))?;

    let mut deb_paths: Vec<String> = actions
        .install
        .iter()
        .map(|x| {
            download_res
                .get(&x.0)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect();

    // Purge stuff
    if !actions.purge.is_empty() {
        let mut cmd: Vec<String> = Vec::new();
        cmd.push("--purge".to_string());
        cmd.append(&mut actions.purge);
        dpkg_run(&cmd, root)?;
    }
    // Remove stuff
    if !actions.remove.is_empty() {
        let mut cmd: Vec<String> = Vec::new();
        cmd.push("--remove".to_string());
        cmd.append(&mut actions.remove);
        dpkg_run(&cmd, root)?;
    }
    // Configure stuff
    if !actions.configure.is_empty() {
        let mut cmd: Vec<String> = Vec::new();
        cmd.push("--configure".to_string());
        cmd.append(&mut actions.configure);
        dpkg_run(&cmd, root)?;
    }
    // Install stuff
    if !deb_paths.is_empty() {
        let mut cmd: Vec<String> = Vec::new();
        cmd.push("--install".to_string());
        cmd.append(&mut deb_paths);
        dpkg_run(&cmd, root)?;
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
