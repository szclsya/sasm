mod blueprint;
mod ignorerule;
pub use blueprint::{Blueprints, PkgRequest};
pub use ignorerule::IgnoreRules;

use anyhow::{bail, Result};
use clap::Parser;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub arch: String,
    pub purge_on_remove: bool,
    pub repo: HashMap<String, RepoConfig>,
}

impl Config {
    pub fn check_sanity(&self) -> Result<()> {
        lazy_static! {
            static ref KEY_FILENAME: Regex = Regex::new("^[a-zA-Z0-9.]+$").unwrap();
        }

        for (name, repo) in &self.repo {
            for key_filename in &repo.keys {
                if !KEY_FILENAME.is_match(key_filename) {
                    bail!(
                        "Invalid character in public key name {} for repo {}",
                        name,
                        key_filename
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RepoConfig {
    pub url: String,
    pub distribution: String,
    pub components: Vec<String>,
    pub keys: Vec<String>,
}

#[derive(Parser)]
#[clap(version = "0.1.0", author = "Leo Shen <i@szclsya.me>")]
pub struct Opts {
    #[clap(long, default_value = "/", about = "Root directory for operation")]
    pub root: PathBuf,
    #[clap(
        long,
        default_value = "etc/omakase/",
        about = "Position of the config folder"
    )]
    pub config_root: PathBuf,
    #[clap(short, long, about = "Print additional debug information")]
    pub verbose: bool,
    #[clap(long, about = "Unpack but not configure desired packages")]
    pub unpack_only: bool,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}

#[derive(Parser)]
pub enum SubCmd {
    #[clap(about = "Install new packages")]
    Install(InstallPkg),
    #[clap(about = "Remove packages")]
    Remove(RemovePkg),
    #[clap(about = "Refresh local package databases")]
    Refresh,
    #[clap(about = "Install and upgrade all packages according to blueprint")]
    Execute,
    #[clap(about = "Alias to Execute")]
    Upgrade,
    #[clap(about = "Search packages from package database")]
    Search(SearchPkg),
}

#[derive(Parser)]
pub struct InstallPkg {
    pub names: Vec<String>,
}

#[derive(Parser)]
pub struct RemovePkg {
    pub names: Vec<String>,
}

#[derive(Parser)]
pub struct SearchPkg {
    pub keyword: String,
}
