mod blueprint;
pub use blueprint::{Blueprints, PkgRequest};

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
#[clap(about, version, author)]
pub struct Opts {
    #[clap(long, default_value = "/", help = "Root directory for operation")]
    pub root: PathBuf,
    #[clap(
        long,
        default_value = "etc/omakase/",
        help = "Position of the config folder"
    )]
    pub config_root: PathBuf,
    #[clap(short, long, help = "Print additional debug information")]
    pub verbose: bool,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}

#[derive(Parser)]
pub enum SubCmd {
    /// Install new packages
    Install(InstallPkg),
    /// Remove packages
    Remove(RemovePkg),
    /// Refresh local package databases
    Refresh,
    /// Install and upgrade all packages according to Blueprint
    Execute,
    /// Alias to Execute
    Upgrade,
    /// Search packages from package database
    Search(SearchPkg),
    /// Search what packages provide a certain file
    Provide(ProvideFile),
    /// Delete local database and package cache
    Clean(CleanConfig),
}

#[derive(Parser)]
pub struct InstallPkg {
    /// Package names or deb file names to install
    #[clap(min_values = 1)]
    pub names: Vec<String>,

    /// Don't install recommended packages
    #[clap(long)]
    pub no_recommends: bool,
    /// Install local debs files rather from the repositories
    #[clap(long)]
    pub local: bool,
}

#[derive(Parser)]
pub struct RemovePkg {
    /// Package names to remove
    #[clap(min_values = 1)]
    pub names: Vec<String>,
    /// Also remove recommended packages
    #[clap(long)]
    pub remove_recommends: bool,
}

#[derive(Parser)]
pub struct SearchPkg {
    /// Search keyword for package name
    pub keyword: String,
}

#[derive(Parser)]
pub struct ProvideFile {
    /// Partial or full path for searching
    pub file: String,
}

#[derive(Parser)]
pub struct CleanConfig {
    /// Remove both package cache and local database
    #[clap(short, long)]
    pub all: bool,
}
