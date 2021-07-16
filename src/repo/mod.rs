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

pub trait Repo {
    fn get_db(&self, reponame: &str) -> Result<Vec<Box<dyn std::io::Read>>>;
}

pub struct NetRepo {
    client: Client,
    arch: Vec<String>,
    repos: HashMap<String, RepoConfig>,
}

impl NetRepo {
    pub fn new(arch: String, repos: HashMap<String, RepoConfig>) -> Self {
        NetRepo {
            client: Client::new(),
            arch: vec![arch, "all".to_string()],
            repos,
        }
    }
}

impl Repo for NetRepo {
    fn get_db(&self, reponame: &str) -> Result<Vec<Box<dyn std::io::Read>>> {
        let repo = match self.repos.get(reponame) {
            Some(repo) => repo,
            None => {
                bail!("No such repo")
            }
        };
        let mut res: Vec<Box<dyn std::io::Read>> = Vec::new();
        for component in &repo.components {
            for arch in &self.arch {
                let url = format!(
                    "{}/dists/{}/{}/binary-{}/Packages",
                    repo.url, repo.distribution, component, arch
                );
                let req = self.client.get(&url).send()?;
                if req.status() == 404 {
                    // It's possible that this repo doesn't have this arch
                    println!("No arch {} for repo {}", arch, reponame);
                    continue;
                } else if req.status() != 200 {
                    bail!("Repo connection error");
                }
                res.push(Box::new(req.bytes()?.reader()));
            }
        }
        Ok(res)
    }
}
