mod actions;
mod cli;
mod db;
mod executor;
mod pool;
mod solver;
mod types;
mod utils;
use types::config::{Blueprints, Config, Opts};

use anyhow::{bail, Context, Result};
use clap::Parser;
use lazy_static::lazy_static;
use std::{
    fs::{read_dir, File},
    io::Read,
    process::exit,
    sync::atomic::{AtomicBool, Ordering},
};

// Initialize writer
lazy_static! {
    static ref WRITER: cli::Writer = cli::Writer::new();
}
// Debug flag
static VERBOSE: AtomicBool = AtomicBool::new(false);
static DPKG_RUNNING: AtomicBool = AtomicBool::new(false);
// Global constants
const DB_KEY_PATH: &str = "etc/omakase/keys";
const DB_CACHE_PATH: &str = "var/cache/omakase/db";
const PKG_CACHE_PATH: &str = "var/cache/omakase/pkgs";
const LOCK_PATH: &str = "var/lib/omakase/lock";
const LOCAL_REPO_PATH: &str = "var/lib/omakase/local_repo";

/// Check if in verbose mode
fn verbose() -> bool {
    crate::VERBOSE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Exit codes:
/// 1 => program screwed up
/// 2 => user cancelled operation
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Initial setup
    let opts: Opts = Opts::parse();
    {
        // Set up SIGINT handler
        let root = opts.root.clone();
        ctrlc::set_handler(move || {
            if DPKG_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                warn!("Cannot interrupt when dpkg is running");
            } else {
                // Thanks to stateless, we can just exit
                utils::lock::unlock(&root).expect("Failed to unlock");
                exit(2);
            }
        })
        .expect("Error setting SIGINT handler");
    }
    // Set-up debug globally
    VERBOSE.store(opts.verbose, Ordering::Relaxed);

    // Run main logic
    if let Err(err) = try_main(&opts).await {
        // Create a new line first, for visual distinction
        WRITER.writeln("", "").ok();
        error!("{}", err.to_string());
        err.chain().skip(1).for_each(|cause| {
            due_to!("{}", cause);
        });
        std::process::exit(1);
    }
}

async fn try_main(opts: &Opts) -> Result<()> {
    // Start reading configs
    let config_root = opts
        .root
        .join(&opts.config_root)
        .canonicalize()
        .context(format!(
            "Failed to find config_root at {}",
            opts.config_root.display()
        ))?;
    if !config_root.is_dir() {
        bail!(
            "Config root does not exist or is not a directory at {}",
            config_root.display()
        );
    }

    let config_path = config_root.join("config.toml");
    // Set-up main config file
    let mut config_file = File::open(&config_path).context(format!(
        "Failed to open config file at {}",
        config_path.display()
    ))?;
    let mut data = String::new();
    config_file
        .read_to_string(&mut data)
        .context("Failed to read config file")?;
    let config: Config = toml::from_str(&data).context("Failed to parse config file")?;
    config.check_sanity()?;

    // Set-up blueprints
    let mut vendor_blueprint_paths = Vec::new();
    let blueprint_d_path = config_root.join("blueprint.d");
    if blueprint_d_path.is_dir() {
        let paths = read_dir(blueprint_d_path).context("Failed to load Blueprint directory")?;
        for path in paths {
            let path = path?;
            let filename = path
                .file_name()
                .to_str()
                .context(format!(
                    "Bad filename in config folder: {}",
                    path.path().display()
                ))?
                .to_owned();
            if filename.ends_with(".blueprint") {
                vendor_blueprint_paths.push(path.path());
            }
        }
    }
    let mut blueprint =
        Blueprints::from_files(config_root.join("blueprint"), &vendor_blueprint_paths)?;

    // Do stuff
    warn!("Omakase is still in early alpha stage. DO NOT use me on production systems!");
    actions::fullfill_command(&config, opts, &mut blueprint).await?;
    // Write back blueprint.
    // They will determine if it really need to write back user blueprint
    blueprint.export()?;

    Ok(())
}
