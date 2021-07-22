mod cli;
mod config;
mod executor;
mod repo;
mod solver;
mod types;

use anyhow::{Context, Result};
use config::Config;
use dialoguer::Confirm;
use std::{fs::File, io::Read, path::PathBuf};

/// Exit codes:
/// 1 => program screwed up
/// 2 => user cancelled operation
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = try_main().await {
        error!("{}", err);
        err.chain().skip(1).for_each(|cause| {
            due_to!("{}", cause);
        });
        std::process::exit(1);
    }
}

async fn try_main() -> Result<()> {
    let config_path = PathBuf::from("/etc/apm/config.toml");
    let mut config_file = File::open(&config_path).context("Failed to open config file")?;
    let mut data = String::new();
    config_file
        .read_to_string(&mut data)
        .context("Failed to read config file")?;
    let config: Config = toml::from_str(&data).context("Failed to parse config file")?;

    info!("Synchronizing package databases...");
    let downloader = executor::download::Downloader::new();
    let mut solver = solver::Solver::new();

    let dbs = repo::get_dbs(&config.repo, &config.arch)?;
    for (baseurl, mut db) in dbs.into_iter() {
        solver::deb::read_deb_db(&mut db, &mut solver.pool, &baseurl)?;
    }
    solver.finalize();

    info!("Resolving dependencies...");
    let res = solver.install(config.wishlist)?;
    // Translating result to list of actions
    let root = PathBuf::from(&config.root);
    let machine_status = executor::MachineStatus::new(&root)?;
    let actions = machine_status.gen_actions(res.as_slice(), config.purge_on_remove);
    if actions.is_empty() {
        success!("There's nothing to do, all wishes has been fulfilled!");
    } else {
        if Confirm::new()
            .with_prompt("Proceed with actions?")
            .interact()?
        {
            // Run it!
            executor::dpkg::execute_pkg_actions(actions, &PathBuf::from(&config.root), &downloader)
                .await?;
        } else {
            std::process::exit(2);
        }
    }

    Ok(())
}
