#![allow(dead_code)]
#![allow(unused_imports)]
// TODO: Remove above allows when the transition is done
mod actions;
mod alpm;
mod config;
mod executor;
mod solver;
mod types;
mod utils;
use config::{Blueprints, Config, Opts};

use anyhow::{bail, Context, Result};
use clap::Parser;
use lazy_static::lazy_static;
use nix::sys::signal;
use std::{
    fs::{read_dir, File},
    io::Read,
    path::Path,
    process::exit,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
};

// Initialize writer
lazy_static! {
    static ref WRITER: utils::cli::Writer = utils::cli::Writer::new();
}
// Debug flag
static VERBOSE: AtomicBool = AtomicBool::new(false);
// Global states
static DPKG_RUNNING: AtomicBool = AtomicBool::new(false);
static LOCKED: AtomicBool = AtomicBool::new(false);
static SUBPROCESS: AtomicI32 = AtomicI32::new(-1);
// Global constants
const DB_KEY_PATH: &str = "etc/sasm/keys";
const DB_CACHE_PATH: &str = "var/cache/sasm/db";
const PKG_CACHE_PATH: &str = "var/cache/sasm/pkgs";
const LOCK_PATH: &str = "var/lib/sasm/lock";
const LOCAL_REPO_PATH: &str = "var/lib/sasm/local_repo";

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
    let mut opts: Opts = Opts::parse();
    // Configure debug globally
    VERBOSE.store(opts.verbose, Ordering::Relaxed);
    // If yes mode is enabled, pager will be disabled
    if opts.yes {
        opts.no_pager = true;
    }

    // Set up SIGINT handler
    {
        let root = opts.root.to_owned();
        ctrlc::set_handler(move || sigint_handler(&root)).expect("Error setting SIGINT handler.");
    }

    // Run main logic
    let exit_code = match try_main(&opts).await {
        Ok(exit_code) => exit_code,
        Err(err) => {
            // Create a new line first, for visual distinction
            WRITER.writeln("", "").ok();
            error!("{}", err.to_string());
            err.chain().skip(1).for_each(|cause| {
                due_to!("{}", cause);
            });
            1
        }
    };

    // Unlock if current process locked
    if LOCKED.load(Ordering::Relaxed) {
        if let Err(e) = utils::lock::unlock(&opts.root) {
            error!("{}", e);
        }
    }

    // Always show cursor, just in case
    let _ = WRITER.show_cursor();

    exit(exit_code);
}

async fn try_main(opts: &Opts) -> Result<i32> {
    // Start reading configs
    let config_root = opts.root.join(&opts.config_root).canonicalize().context(format!(
        "Failed to find config_root in Sasm configuration file {} .",
        opts.config_root.display()
    ))?;
    if !config_root.is_dir() {
        bail!(
            "Configuration root (config_root) does not exist or is not a directory at {} .",
            config_root.display()
        );
    }

    let config_path = config_root.join("config.toml");
    // Set-up main config file
    let mut config_file = File::open(&config_path)
        .context(format!("Failed to open configuration file {} .", config_path.display()))?;
    let mut data = String::new();
    config_file.read_to_string(&mut data).context("Failed to read configuration file.")?;
    let config: Config = toml::from_str(&data).context("Failed to parse configuration file.")?;

    // Set-up blueprints
    let mut vendor_blueprint_paths = Vec::new();
    let blueprint_d_path = config_root.join("blueprint.d");
    if blueprint_d_path.is_dir() {
        let paths = read_dir(blueprint_d_path).context("Failed to load Blueprint directory.")?;
        for path in paths {
            let path = path?;
            let filename = path
                .file_name()
                .to_str()
                .context(format!(
                    "Bad filename in configuration folder: {} .",
                    path.path().display()
                ))?
                .to_owned();
            if filename.ends_with(".blueprint") {
                vendor_blueprint_paths.push(path.path());
            }
        }
    }
    let mut blueprint =
        Blueprints::from_files(config_root.join("user.blueprint"), &vendor_blueprint_paths)?;

    // Do stuff
    warn!("Sasm is currently under construction and active testing. Proceed with caution on production systems!");
    let cancelled = actions::fullfill_command(&config, opts, &mut blueprint).await?;
    if !cancelled {
        // Write back blueprint.
        // They will determine if it really need to write back user blueprint
        blueprint.export()?;
        Ok(0)
    } else {
        // User cancelled operation. Don't write back blueprint
        Ok(2)
    }
}

fn sigint_handler(root: &Path) {
    if crate::DPKG_RUNNING.load(Ordering::Relaxed) {
        warn!("You may not interrupt Sasm when dpkg is running.");
        // Don't exit. Important things are happening
        return;
    }

    // Kill subprocess
    let subprocess_pid = SUBPROCESS.load(Ordering::Relaxed);
    if subprocess_pid > 0 {
        let pid = nix::unistd::Pid::from_raw(subprocess_pid);
        signal::kill(pid, signal::SIGTERM).expect("Failed to kill child process.");
    }

    // Dealing with lock
    if LOCKED.load(Ordering::Relaxed) {
        utils::lock::unlock(root).expect("Failed to unlock instance.");
    }

    // Show cursor before exiting.
    // This is not a big deal so we won't panic on this.
    let _ = WRITER.show_cursor();
    std::process::exit(2);
}
