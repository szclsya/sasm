mod blueprint;
pub use blueprint::{Blueprints, PkgRequest};
mod repo;
pub use repo::RepoConfig;

use crate::warn;

use anyhow::{bail, Context, Result};
use clap::Parser;
use console::style;
use serde::{Deserialize, Serialize, Serializer};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub arch: String,
    #[serde(serialize_with = "ordered_map")]
    pub repo: HashMap<String, RepoConfig>,
}

fn ordered_map<S>(value: &HashMap<String, RepoConfig>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}

#[derive(Parser)]
#[clap(about, version, author)]
pub struct Opts {
    #[clap(
        display_order = 1,
        long,
        default_value = "/",
        help = "Root directory for operation"
    )]
    pub root: PathBuf,
    #[clap(
        display_order = 2,
        long,
        default_value = "etc/sasm/",
        help = "Position of the config folder"
    )]
    pub config_root: PathBuf,
    #[clap(display_order = 3, long, help = "Say yes to every prompt")]
    pub yes: bool,
    #[clap(
        display_order = 4,
        short,
        long,
        help = "Print additional debug information"
    )]
    pub verbose: bool,
    #[clap(display_order = 5, long, help = "Don't pipe long output into a pager")]
    pub no_pager: bool,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}

#[derive(Parser)]
pub enum SubCmd {
    /// Install and upgrade all packages according to Blueprint
    #[clap(display_order = 4, aliases = &["upgrade"])]
    Execute,
    /// Delete local package cache (optionally metadata cache)
    #[clap(display_order = 21)]
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
    /// Install local package files rather from the repositories
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
pub struct PickPkg {
    /// Package names to pick version
    pub name: String,
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
    /// Search binary files only
    #[clap(long)]
    pub bin: bool,
}

#[derive(Parser)]
pub struct CleanConfig {
    /// Remove both package cache and local database
    #[clap(short, long)]
    pub all: bool,
}

#[derive(Parser)]
pub struct DownloadPkg {
    /// Name of package
    pub pkgname: String,
    /// Use latest version automatically
    #[clap(long)]
    pub latest: bool,
}
