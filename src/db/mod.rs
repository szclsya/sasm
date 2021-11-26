mod verify;

use crate::{
    types::{config::RepoConfig, Checksum},
    utils::downloader::{DownloadJob, Downloader, Compression},
    warn
};
use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug)]
pub struct LocalDb {
    // root directory for dbs
    root: PathBuf,
    // directory that stores repo public keys
    key_root: PathBuf,
    arch: String,
    repos: HashMap<String, RepoConfig>,
}

impl LocalDb {
    pub fn new(
        root: PathBuf,
        key_root: PathBuf,
        repos: HashMap<String, RepoConfig>,
        arch: &str,
    ) -> Self {
        LocalDb {
            root,
            key_root,
            arch: arch.to_owned(),
            repos,
        }
    }

    pub fn get(&self, name: &str) -> Result<Vec<(String, PathBuf)>> {
        let repo = match self.repos.get(name) {
            Some(repo) => repo,
            None => bail!("Repo with name {} not found", name),
        };

        let mut files: Vec<(String, PathBuf)> = Vec::new();
        for component in &repo.components {
            // First prepare arch-specific repo
            let arch = self.root.join(format!(
                "{}/Packages_{}_{}_{}",
                &name, &repo.distribution, component, &self.arch
            ));
            if !arch.is_file() {
                bail!("Local database is corrupted or out-of-date");
            }
            files.push((repo.url.clone(), self.root.join(arch)));
            // Then prepare noarch repo, if exists
            let noarch = self.root.join(format!(
                "{}/Packages_{}_{}_{}",
                &name, &repo.distribution, component, "all"
            ));
            if noarch.is_file() {
                files.push((repo.url.clone(), self.root.join(noarch)));
            }
        }

        Ok(files)
    }

    // Get (BaseURL, FilePath) of all configured repos
    pub fn get_all(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut res = Vec::new();
        for repo in &self.repos {
            res.append(&mut self.get(repo.0)?);
        }
        Ok(res)
    }

    pub async fn update(&self, downloader: &Downloader) -> Result<()> {
        // HashMap<RepoName, HashMap<url, (size, checksum)>>
        let mut dbs: HashMap<String, HashMap<String, (u64, Checksum)>> = HashMap::new();
        // Step 1: Download InRelease for each repo
        let inrelease_urls: Vec<DownloadJob> = self
            .repos
            .iter()
            .map(|(name, repo)| DownloadJob {
                url: format!("{}/dists/{}/InRelease", repo.url, repo.distribution),
                filename: Some(format!("InRelease_{}", name)),
                size: None,
                checksum: None,
                compression: Compression::None,
            })
            .collect();
        downloader.fetch(inrelease_urls, &self.root).await?;

        // Step 2: Verify InRelease with PGP
        for (name, repo) in self.repos.iter() {
            let inrelease_path = self.root.join(format!("InRelease_{}", name));
            let inrelease_contents = std::fs::read(inrelease_path)?;
            let bytes = bytes::Bytes::from(inrelease_contents);
            let res = verify::verify_inrelease(&self.key_root, &repo.keys, bytes)
                .context(format!("Failed to verify metadata for repository {}", name))?;
            let repo_dbs = parse_inrelease(&res)
                .context(format!("Failed to parse metadata for repository {}", name))?;
            dbs.insert(name.to_owned(), repo_dbs);
        }

        // Step 3: Download deb dbs
        let mut dbs_to_download = Vec::new();
        for (name, repo) in &self.repos {
            // Create sub-directory for each repo
            let db_subdir = self.root.join(name);
            if !db_subdir.is_dir() {
                std::fs::create_dir(&self.root.join(name))?;
            }

            for component in &repo.components {
                let pre_download_count = dbs_to_download.len();
                let possible_archs = vec![self.arch.to_owned(), "all".to_owned()];
                for arch in possible_archs {
                    let rel_url = format!("{}/binary-{}/Packages.xz", component, arch);
                    if let Some(db_meta) = dbs.get(name).unwrap().get(&rel_url) {
                        let filename = format!(
                            "{}/Packages_{}_{}_{}",
                            &name, &repo.distribution, &component, arch
                        );
                        dbs_to_download.push(DownloadJob {
                            url: format!("{}/dists/{}/{}", repo.url, repo.distribution, rel_url),
                            filename: Some(filename),
                            size: Some(db_meta.0),
                            checksum: Some(db_meta.1.clone()),
                            compression: Compression::Xz,
                        });
                    }
                }

                if pre_download_count == dbs_to_download.len() {
                    warn!("No repository available for {}/{}", name, component);
                    warn!("Please check if this repo have packages for {} architecture", self.arch);
                }
            }
        }

        // Step 4: Call Downloader to down them all!
        // The downloader will verify the checksum for us
        downloader.fetch(dbs_to_download, &self.root).await?;

        Ok(())
    }
}

fn parse_inrelease(s: &str) -> Result<HashMap<String, (u64, Checksum)>> {
    lazy_static! {
        static ref CHKSUM: Regex =
            Regex::new("^(?P<chksum>[0-9a-z]+) +(?P<size>[0-9]+) +(?P<path>.+)$").unwrap();
    }

    let mut dbs: HashMap<String, (u64, Checksum)> = HashMap::new();
    let paragraphs = debcontrol::parse_str(s).unwrap();
    for p in paragraphs {
        for field in p.fields {
            if field.name == "SHA256" || field.name == "SHA512" {
                // Parse the checksum fields
                for line in field.value.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    let captures = match CHKSUM.captures(line) {
                        Some(c) => c,
                        None => {
                            bail!("Malformed InRelease");
                        }
                    };
                    let rel_path = captures.name("path").unwrap().as_str().to_string();
                    let size: u64 = captures.name("size").unwrap().as_str().parse()?;
                    let chksum = {
                        match field.name {
                            "SHA256" => Checksum::from_sha256_str(
                                captures.name("chksum").unwrap().as_str(),
                            )?,
                            "SHA512" => Checksum::from_sha512_str(
                                captures.name("chksum").unwrap().as_str(),
                            )?,
                            // This should never happen
                            _ => panic!(),
                        }
                    };
                    dbs.insert(rel_path, (size, chksum));
                }
                return Ok(dbs);
            }
        }
    }

    bail!("No db hash found in InRelease. Supported Hash: SHA256")
}
