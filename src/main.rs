mod actions;
mod cli;
mod db;
mod executor;
mod solver;
mod types;
use types::config::{Config, Opts, Wishlist};

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
    let wishlist_path = config_root.join("wishlist");

    // Read config
    let mut config_file = File::open(&config_path).context(format!(
        "Failed to open config file at {}",
        config_path.display()
    ))?;
    let mut data = String::new();
    config_file
        .read_to_string(&mut data)
        .context("Failed to read config file")?;
    let config: Config = toml::from_str(&data).context("Failed to parse config file")?;

    // Read wishlist
    let mut wishlist = Wishlist::from_file(&wishlist_path)?;

    // Do stuff
    warn!("apm is still in early alpha stage. DO NOT use me on production systems!");
    let wishlist_modified = actions::fullfill_command(&config, &opts, &mut wishlist).await?;

    // Write back wishlist, if the operations involves modifying it
    if wishlist_modified {
        let new_wishlist = wishlist.export();
        let wishlist_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&wishlist_path)?;
        wishlist_file.set_len(0)?;
        wishlist_file
            .write_all_at(&new_wishlist.into_bytes(), 0)
            .context(format!(
                "Failed to write to wishlist file at {}",
                wishlist_path.display()
            ))?;
    }

    Ok(())
}
