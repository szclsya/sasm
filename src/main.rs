mod actions;
mod cli;
mod db;
mod executor;
mod solver;
mod types;
use types::config::{Blueprint, Config, Opts};

use anyhow::{bail, Context, Result};
use clap::Clap;
use lazy_static::lazy_static;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::FileExt,
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

    let config_path = config_root.join("apm.toml");
    let blueprint_path = config_root.join("blueprint");

    // Read config
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

    // Read blueprint
    let mut blueprint = Blueprint::from_file(&blueprint_path)?;

    // Do stuff
    warn!("apm is still in early alpha stage. DO NOT use me on production systems!");
    let blueprint_modified = actions::fullfill_command(&config, &opts, &mut blueprint).await?;

    // Write back blueprint, if the operations involves modifying it
    if blueprint_modified {
        let new_blueprint = blueprint.export();
        let blueprint_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&blueprint_path)?;
        blueprint_file.set_len(0)?;
        blueprint_file
            .write_all_at(&new_blueprint.into_bytes(), 0)
            .context(format!(
                "Failed to write to blueprint file at {}",
                blueprint_path.display()
            ))?;
    }

    Ok(())
}
