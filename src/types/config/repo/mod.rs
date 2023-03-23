use anyhow::{ bail, Result };
use serde::{Deserialize, Serialize, Serializer};
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RepoConfig {
    pub source: Mirror,
    pub keys: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Mirror {
    Simple(String),
    MirrorList(PathBuf),
}

impl RepoConfig {
    /// Check if there's some mirror available
    pub fn check_sanity(&self) -> Result<()> {
        // TODO: Implement mirrorlist
        if matches!(self.source, Mirror::MirrorList(_)) {
            bail!("mirrorlist not supported yet!")
        }

        Ok(())
    }

    /// Get base urls for all repositories
    /// Returns a list of possible urls for the repository
    pub fn get_url(&self, name: &str, arch: &str) -> Result<String> {
        let mut url = match &self.source {
            Mirror::Simple(m) => {
                let mut url = m.clone();
                normalize_mirror_url(&mut url);
                url
            }
            Mirror::MirrorList(path) => {
                unimplemented!()
            }
        };

        // Replace variables
        // $repo: Repository name
        // $arch: Current system architecture
        url = url.replace("$repo", name);
        url = url.replace("$arch", arch);
        Ok(url)
    }
}

fn normalize_mirror_url(url: &mut String) {
    if url.ends_with('/') {
        url.pop();
    }
}

pub enum MirrorlistLine {
}
