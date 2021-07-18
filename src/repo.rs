use anyhow::{bail, Result};
use bytes::Buf;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct RepoConfig {
    url: String,
    distribution: String,
    components: Vec<String>,
}

pub fn get_dbs(
    repos: &HashMap<String, RepoConfig>,
    arch: &str,
) -> Result<Vec<(String, Box<dyn std::io::Read>)>> {
    let client = Client::new();

    let archs = vec![arch.to_string(), "all".to_string()];
    let mut res: Vec<(String, Box<dyn std::io::Read>)> = Vec::new();
    for repo in repos.values() {
        for component in &repo.components {
            for arch in &archs {
                let url = format!(
                    "{}/dists/{}/{}/binary-{}/Packages",
                    repo.url, repo.distribution, component, arch
                );
                let req = client.get(&url).send()?;
                if req.status() == 404 {
                    // It's possible that this repo doesn't have this arch
                    bail!("Repo db not found: {}", &url);
                } else if req.status() != 200 {
                    bail!("Repo connection error");
                }
                res.push((repo.url.to_string(), Box::new(req.bytes()?.reader())));
            }
        }
    }
    Ok(res)
}
