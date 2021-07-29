mod verify;

use crate::{
    config::RepoConfig,
    executor::download::{DownloadJob, Downloader},
    types::Checksum,
};
use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;
use std::{collections::HashMap, path::Path};

pub async fn get_dbs(
    repos: &HashMap<String, RepoConfig>,
    arch: &str,
    root: &Path,
    downloader: &Downloader,
) -> Result<Vec<(String, PathBuf)>> {
    // Turn real url back to baseurl
    let mut url_to_baseurl = HashMap::new();
    // Add noarch
    let archs = vec![arch.to_string(), "all".to_string()];

    // Verify signatures of the repos
    let mut dbs: HashMap<String, (u64, Checksum)> = HashMap::new();
    for (name, repo) in repos.iter() {
        let root = format!("{}/dists/{}", repo.url, repo.distribution);
        // Ger InRelease to verify signature of Packages db
        let inrelease_url = format!("{}/InRelease", root);
        let inrelease_contents = reqwest::get(&inrelease_url)
            .await
            .context(format!("Failed to get metadata for repository {}", name))?
            .bytes()
            .await?;
        let res = verify::verify_inrelease(&repo.certs, inrelease_contents)
            .context(format!("Failed to verify metadata for repository {}", name))?;
        parse_inrelease(&res, &root, &mut dbs)
            .context(format!("Failed to parse metadata for repository {}", name))?;
    }

    let mut dbs_to_download = Vec::new();
    for (name, repo) in repos.iter() {
        for component in &repo.components {
            for arch in &archs {
                let filename =
                    format! {"Packages_{}_{}_{}_{}", &name, &repo.distribution, &component, &arch};
                let url = format!(
                    "{}/dists/{}/{}/binary-{}/Packages",
                    repo.url, repo.distribution, component, arch
                );
                let db_meta = match dbs.get(&url) {
                    Some(m) => m,
                    None => {
                        bail!("Repository {} doesn't contain necessary dbs", name);
                    }
                };
                dbs_to_download.push(DownloadJob {
                    url: url.clone(),
                    filename: Some(filename),
                    size: Some(db_meta.0),
                    checksum: Some(db_meta.1.clone()),
                });
                // Record url->baseurl mapping
                url_to_baseurl.insert(url, &repo.url);
            }
        }
    }

    // Call Downloader to down them all!
    let paths = downloader
        .fetch(dbs_to_download, &root.join("var/cache/apm/db"))
        .await?;
    let mut res: Vec<(String, PathBuf)> = Vec::new();
    for (url, path) in paths.into_iter() {
        res.push((url_to_baseurl.get(&url).unwrap().to_string(), path));
    }
    Ok(res)
}

fn parse_inrelease(s: &str, root: &str, dbs: &mut HashMap<String, (u64, Checksum)>) -> Result<()> {
    lazy_static! {
        static ref CHKSUM: Regex =
            Regex::new("^(?P<chksum>[0-9a-z]+) +(?P<size>[0-9]+) +(?P<path>.+)$").unwrap();
    }

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
                    let rel_path = captures.name("path").unwrap().as_str();
                    let real_path = format!("{}/{}", root, rel_path);
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
                    dbs.insert(real_path, (size, chksum));
                }
                return Ok(());
            }
        }
    }

    bail!("No db hash found in InRelease. Supported Hash: SHA256")
}
