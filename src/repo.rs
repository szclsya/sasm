use crate::{config::RepoConfig, executor::download::Downloader};
use anyhow::{Result};
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

    let mut dbs = Vec::new();
    for (name, repo) in repos.iter() {
        for component in &repo.components {
            for arch in &archs {
                let filename =
                    format! {"{}_{}_{}_{}", &name, &repo.distribution, &component, &arch};
                let url = format!(
                    "{}/dists/{}/{}/binary-{}/Packages",
                    repo.url, repo.distribution, component, arch
                );
                // We don't know the size, so None
                dbs.push((url.clone(), Some(filename), None));
                // Record url->baseurl mapping
                url_to_baseurl.insert(url, &repo.url);
            }
        }
    }

    // Call Downloader to down them all!
    let paths = downloader
        .fetch(dbs, &root.join("var/cache/apm/db"))
        .await?;
    // Open files for read
    let mut res: Vec<(String, PathBuf)> = Vec::new();
    for (url, path) in paths.into_iter() {
        res.push((url_to_baseurl.get(&url).unwrap().to_string(), path));
    }
    Ok(res)
}
