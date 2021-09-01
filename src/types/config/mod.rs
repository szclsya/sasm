mod wishlist;
pub use wishlist::{PkgRequest, Wishlist};

use clap::Clap;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub arch: String,
    pub purge_on_remove: bool,
    pub repo: HashMap<String, RepoConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RepoConfig {
    pub url: String,
    pub distribution: String,
    pub components: Vec<String>,
    pub certs: Vec<PathBuf>,
}

#[derive(Clap)]
#[clap(version = "0.1.0", author = "Leo Shen <i@szclsya.me>")]
pub struct Opts {
    #[clap(long, default_value = "/")]
    pub root: PathBuf,
    #[clap(long, default_value = "etc/apm/")]
    pub config_root: PathBuf,
    #[clap(long)]
    pub unpack_only: bool,
    #[clap(subcommand)]
    pub subcmd: Option<SubCmd>,
}

#[derive(Clap)]
pub enum SubCmd {
    Add(AddPkg),
    Rm(RmPkg),
}

#[derive(Clap)]
pub struct AddPkg {
    pub name: String,
}

#[derive(Clap)]
pub struct RmPkg {
    pub name: String,
}
