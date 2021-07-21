use crate::types::VersionRequirement;

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Config {
    pub arch: String,
    #[serde(default = "default_root")]
    pub root: String,
    pub purge_on_remove: bool,
    pub repo: HashMap<String, RepoConfig>,
    pub wishlist: HashMap<String, VersionRequirement>,
}

#[derive(Deserialize)]
pub struct RepoConfig {
    pub url: String,
    pub distribution: String,
    pub components: Vec<String>,
}

#[inline]
fn default_root() -> String {
    "/".to_string()
}
