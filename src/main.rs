mod actions;
mod cli;
mod db;
mod executor;
mod solver;
mod types;
use types::config::{Blueprints, Config, Opts};

use anyhow::{bail, Context, Result};
use clap::Parser;
use lazy_static::lazy_static;
use std::{
    fs::{File, read_dir},
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
};

// Initialize writer
lazy_static! {
    static ref WRITER: cli::Writer = cli::Writer::new();
}
// Debug flag
static DEBUG: AtomicBool = AtomicBool::new(false);

/// Exit codes:
/// 1 => program screwed up
/// 2 => user cancelled operation
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = try_main().await {
        error!("{}", err.to_string());
        err.chain().skip(1).for_each(|cause| {
            due_to!("{}", cause);
        });
        std::process::exit(1);
    }
}

async fn try_main() -> Result<()> {
    // Initial setup
    let opts: Opts = Opts::parse();
    // Set-up debug globally
    DEBUG.store(opts.debug, Ordering::Relaxed);
    let config_root = opts
        .root
        .join(&opts.config_root)
        .canonicalize()
        .context("Failed to find config_root")?;
    if !config_root.is_dir() {
        bail!(
            "Config root does not exist or is not a directory at {}",
            config_root.display()
        );
    }

    let config_path = config_root.join("config.toml");
    // Compose blueprints
    let mut vendor_blueprint_paths = Vec::new();
    let blueprint_d_path = config_root.join("blueprint.d");
    if blueprint_d_path.is_dir() {
        let paths = read_dir(blueprint_d_path).context("Failed to load blueprint directory")?;
        for path in paths {
            let path = path?;
            let filename = path.file_name().to_str()
                .context(format!("Bad filename in config folder: {}", path.path().display()))?.to_owned();
            if filename.ends_with(".blueprint") {
                vendor_blueprint_paths.push(path.path());
            }
        }
    }

    let mut blueprint = Blueprints::from_files(config_root.join("blueprint"), &vendor_blueprint_paths)?;

    // Read configs
    let mut config_file = File::open(&config_path).context(format!(
        "Failed to open config file at {}",
        config_path.display()
    ))?;
    let mut data = String::new();
    config_file
        .read_to_string(&mut data)
        .context("Failed to read config file")?;
    let mut config: Config = toml::from_str(&data).context("Failed to parse config file")?;
    // Cert paths are relative to config root
    for repo in config.repo.iter_mut() {
        for cert_path in repo.1.certs.iter_mut() {
            let config_root = opts.root.join(&opts.config_root);
            *cert_path = config_root.join(&cert_path);
        }
    }

    // Do stuff
    warn!("Omakase is still in early alpha stage. DO NOT use me on production systems!");
    let blueprint_modified = actions::fullfill_command(&config, &opts, &mut blueprint).await?;

    // Write back blueprint, if the operations involves modifying it
    if blueprint_modified {
        blueprint.export()?;
    }

    Ok(())
}
