mod blueprint;
pub use blueprint::{Blueprints, PkgRequest};

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
    pub r#unsafe: Option<UnsafeConfig>,
}

fn ordered_map<S>(value: &HashMap<String, RepoConfig>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct UnsafeConfig {
    #[serde(default)]
    pub purge_on_remove: bool,
    #[serde(default)]
    pub unsafe_io: bool,
    #[serde(default)]
    pub allow_remove_essential: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RepoConfig {
    pub source: Mirror,
    pub distribution: String,
    pub components: Vec<String>,
    pub keys: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Mirror {
    Simple(String),
    MirrorList {
        mirrorlist: PathBuf,
        preferred: String,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MirrorList(HashMap<String, MirrorMeta>);

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MirrorMeta {
    pub desc: String,
    pub url: String,
}

impl Config {
    pub fn check_sanity(&self) -> Result<()> {
        for (name, repo) in &self.repo {
            // Check public key names
            for key in &repo.keys {
                if key.contains(|c| !key_filename_char(c)) {
                    bail!("Invalid character in public key filename {name} for repository {key}.",);
                }
            }
            repo.check_sanity()?;
        }

        Ok(())
    }
}

fn key_filename_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'
}

impl RepoConfig {
    /// Check if there's some mirror available
    pub fn check_sanity(&self) -> Result<()> {
        // If we are using MirrorList, test-parse here
        if let Mirror::MirrorList {
            preferred: _,
            mirrorlist,
        } = &self.source
        {
            let path = mirrorlist;
            if !path.is_absolute() {
                bail!(
                    "Path to MirrorList must be absolute! Got: {}.",
                    style(path.display()).bold()
                );
            }
            if !path.is_file() {
                bail!("MirrorList {} is not a file!", style(path.display()).bold());
            }
            let content = fs::read_to_string(&path).context(format!(
                "Failed to read MirrorList file {}!",
                path.display()
            ))?;
            let mirrorlist: MirrorList = serde_yaml::from_str(&content)
                .context(format!("Malformed MirrorList file {}!", path.display()))?;
            // Check if there's at least something to work with
            if mirrorlist.0.is_empty() {
                bail!("MirrorList {} doesn't contain any mirror!", path.display());
            }
        }

        Ok(())
    }

    /// Get the first choice mirror
    pub fn get_url(&self) -> Result<String> {
        let url = match &self.source {
            Mirror::Simple(m) => {
                let mut url = m.clone();
                // Add `debs`
                normalize_mirror_url(&mut url);
                url
            },
            Mirror::MirrorList {
                preferred,
                mirrorlist: _,
            } => {
                let mirrors = self.get_mirrors()?;
                // Check if the preferred mirror exists
                if let Some(mirror) = mirrors.get(preferred) {
                    mirror.url.clone()
                } else {
                    bail!(
                        "Preferred mirror {} doesn't exist in MirrorList!",
                        preferred
                    )
                }
            }
        };

        Ok(url)
    }

    pub fn get_mirrors(&self) -> Result<HashMap<String, MirrorMeta>> {
        if let Mirror::MirrorList {
            preferred: _,
            mirrorlist,
        } = &self.source
        {
            let path = mirrorlist;
            let content = fs::read_to_string(&path).context(format!(
                "Failed to read MirrorList file {}!",
                path.display()
            ))?;
            let mut mirrorlist: MirrorList = serde_yaml::from_str(&content)
                .context(format!("Malformed MirrorList file {}!", path.display()))?;
            for mirror in &mut mirrorlist.0 {
                let url = &mut mirror.1.url;
                normalize_mirror_url(url);
            }
            Ok(mirrorlist.0)
        } else {
            bail!("Cannot get mirrors for simple mirror!");
        }
    }
}

fn normalize_mirror_url(url: &mut String) {
    // Add `debs`
    if !url.ends_with('/') {
        url.push('/');
    }
    url.push_str("debs");
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
        default_value = "etc/omakase/",
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
    /// Install new packages
    #[clap(display_order = 1)]
    Install(InstallPkg),
    /// Remove packages
    #[clap(display_order = 2, aliases = &["purge", "autoremove"])]
    Remove(RemovePkg),
    /// Pick a specific version of a package
    #[clap(display_order = 3)]
    Pick(PickPkg),
    /// Refresh local package databases
    #[clap(display_order = 5, aliases = &["update"])]
    Refresh,
    /// Install and upgrade all packages according to Blueprint
    #[clap(display_order = 4, aliases = &["upgrade", "full-upgrade", "dist-upgrade"])]
    Execute,
    /// Search packages from package database
    #[clap(display_order = 11)]
    Search(SearchPkg),
    /// Search what packages provide a certain file
    #[clap(display_order = 12)]
    Provide(ProvideFile),
    /// Delete local package cache (optionally metadata cache)
    #[clap(display_order = 21)]
    Clean(CleanConfig),
    /// Benchmark and pick optimal mirrors
    #[clap(display_order = 31)]
    Bench,
    /// Download a package from remote repository
    #[clap(display_order = 32)]
    Download(DownloadPkg),
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
