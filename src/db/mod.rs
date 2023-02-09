//mod verify;

use crate::{
    info,
    types::{config::RepoConfig, Checksum},
    utils::downloader::{Compression, DownloadJob, Downloader},
};
use anyhow::{bail, Result};
use console::style;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug)]
pub struct LocalDb {
    // root directory for dbs
    root: PathBuf,
    arch: String,
    repos: HashMap<String, RepoConfig>,
}

impl LocalDb {
    pub fn new(
        root: PathBuf,
        repos: HashMap<String, RepoConfig>,
        arch: &str,
    ) -> Self {
        LocalDb {
            root,
            arch: arch.to_owned(),
            repos,
        }
    }

    /// Get the remote (relative) path and local path for a repository
    pub fn get_package_db(&self, name: &str) -> Result<(String, PathBuf)> {
        let repo = match self.repos.get(name) {
            Some(repo) => repo,
            None => bail!("Repository with name {} not found.", name),
        };

        let arch = &self.arch;
        let remote_relative_path = format!("/{0}/os/{1}/{0}.db", name, self.arch);
        let local_path = self.root.join(self.root.join(format!("{}.db", name)));

        Ok((remote_relative_path, local_path))
    }

    // Get (BaseURL, FilePath) of all configured repos
    pub fn get_all_package_db(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut res = Vec::new();
        for repo in &self.repos {
            res.push(self.get_package_db(repo.0)?);
        }
        Ok(res)
    }

    pub fn get_contents_db(&self, name: &str) -> Result<(String, PathBuf)> {
        let repo = match self.repos.get(name) {
            Some(repo) => repo,
            None => bail!("Repository with name {} not found.", name),
        };

        let arch = &self.arch;
        let remote_relative_path = format!("/{0}/os/{1}/{0}.files", name, self.arch);
        let local_path = self.root.join(self.root.join(format!("{}.files", name)));

        Ok((remote_relative_path, local_path))
    }

    // Get (BaseURL, FilePath) of all configured repos
    pub fn get_all_contents_db(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut res = Vec::new();
        for repo in &self.repos {
            res.push(self.get_contents_db(repo.0)?);
        }
        Ok(res)
    }

    pub async fn update(&self, downloader: &Downloader) -> Result<()> {
        info!("Refreshing local repository metadata...");

        let package_dbs = self.get_all_package_db()?;
        let mut download_jobs = Vec::with_capacity(package_dbs.len());
        for (name, repo) in &self.repos {
            let (remote_path, local_path) = self.get_package_db(&name)?;
            if local_path.is_file() {
                // Calculate old checksum
                let old_checksum = Checksum::from_file_sha256(&local_path)?;
                download_jobs.push(DownloadJob {
                    url: format!("{}/{}", repo.get_url()?, remote_path),
                    description: Some(format!("Package database for {}", style(name).bold())),
                    filename: Some(format!("{}.db", name)),
                    size: None,
                    compression: Compression::None(Some(old_checksum)),
                })
            }
        }

        // The downloader will verify the checksum for us
        downloader.fetch(download_jobs, &self.root, false).await?;

        Ok(())
    }
}
