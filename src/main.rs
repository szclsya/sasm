mod cli;
mod config;
mod executor;
mod repo;
mod solver;
mod types;
use config::{Config, Opts};

use anyhow::{Context, Result};
use clap::Clap;
use dialoguer::Confirm;
use lazy_static::lazy_static;
use std::{fs::File, io::Read};

// Initialize writer
lazy_static! {
    static ref WRITER: cli::Writer = cli::Writer::new();
}

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
    let config_path = opts.root.join(&opts.config);
    let mut config_file = File::open(&config_path).context(format!(
        "Failed to open config file at {}",
        config_path.display()
    ))?;
    let mut data = String::new();
    config_file
        .read_to_string(&mut data)
        .context("Failed to read config file")?;
    let config: Config = toml::from_str(&data).context("Failed to parse config file")?;

    // May the work begin!
    warn!("apm is still in early alpha stage. DO NOT use me on production systems!");
    info!("Synchronizing package databases...");
    let downloader = executor::download::Downloader::new();
    let mut solver = solver::Solver::new();

    let dbs = repo::get_dbs(&config.repo, &config.arch, &opts.root, &downloader)
        .await
        .context("Failed to fetch dpkg databases")?;
    for (baseurl, db) in dbs.into_iter() {
        solver::deb::read_deb_db(&db, &mut solver.pool, &baseurl)?;
    }
    solver.finalize();

    info!("Resolving dependencies...");
    let res = solver.install(config.wishlist)?;
    // Translating result to list of actions
    let root = opts.root.clone();
    let machine_status = executor::MachineStatus::new(&root)?;
    let actions = machine_status.gen_actions(res.as_slice(), config.purge_on_remove);
    if actions.is_empty() {
        success!("There's nothing to do, all wishes has been fulfilled!");
    } else {
        info!("These following actions will be performed:");
        actions.show();
        if Confirm::new()
            .with_prompt(format!("{}{}", cli::gen_prefix(""), "Proceed?"))
            .interact()?
        {
            // Run it!
            executor::dpkg::execute_pkg_actions(actions, &opts.root, &downloader).await?;
        } else {
            std::process::exit(2);
        }
    }

    Ok(())
}
