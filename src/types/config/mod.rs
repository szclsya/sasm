mod blueprint;
mod ignorerules;
pub use blueprint::{Blueprints, PkgRequest};
pub use ignorerules::IgnoreRules;

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
    #[clap(long, help = "Unpack but not configure desired packages")]
    pub unpack_only: bool,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}

#[derive(Parser)]
pub enum SubCmd {
    /// Install new packages
    Install(InstallPkg),
    /// Remove packages
    Remove(RemovePkg),
    /// Manipulate and inspect IgnoreRules tables
    #[clap(subcommand)]
    Ignore(IgnorePkg),
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
    pub names: Vec<String>,

    /// Don't install recommended packages
    #[clap(long)]
    pub no_recommends: bool,
    /// da
    #[clap(long)]
    pub local: bool,
}

#[derive(Parser)]
pub struct RemovePkg {
    pub names: Vec<String>,
    /// Also remove recommended packages"
    #[clap(long)]
    pub remove_recommends: bool,
}

#[derive(Parser)]
pub enum IgnorePkg {
    /// Add rules to user IgnoreRules table
    Add(ModifyIgnore),
    /// Remove rules from user IgnoreRules table
    Remove(ModifyIgnore),
    /// Show all IgnoreRules (including user and vendor)
    Show,
}

#[derive(Parser)]
pub struct ModifyIgnore {
    pub rules: Vec<String>,
}

#[derive(Parser)]
pub struct SearchPkg {
    pub keyword: String,
}

#[derive(Parser)]
pub struct ProvideFile {
    pub file: String,
}

#[derive(Parser)]
pub struct CleanConfig {
    /// Remove both package cache and local database
    #[clap(short, long)]
    pub all: bool,
}
