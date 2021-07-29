use crate::types::VersionRequirement;

use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize)]
pub struct Config {
    pub arch: String,
    #[serde(default = "default_root")]
    pub root: PathBuf,
    pub purge_on_remove: bool,
    pub repo: HashMap<String, RepoConfig>,
    pub wishlist: HashMap<String, VersionRequirement>,
}

#[derive(Deserialize)]
pub struct RepoConfig {
    pub url: String,
    pub distribution: String,
    pub components: Vec<String>,
    pub certs: Vec<PathBuf>,
}

#[inline]
fn default_root() -> PathBuf {
    PathBuf::from("/")
}
