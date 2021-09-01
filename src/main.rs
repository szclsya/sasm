mod cli;
mod executor;
mod db;
mod solver;
mod types;
use types::{
    config::{Config, Opts, SubCmd, Wishlist},
    PkgActionModifier,
};

use anyhow::{bail, Context, Result};
use clap::Clap;
use dialoguer::Confirm;
use lazy_static::lazy_static;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::FileExt,
};

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
    let mut wishlist_modified = false;
    match &opts.subcmd {
        None => fullfill_wishs(&config, &opts, &wishlist).await?,
        Some(_) => {
            wishlist_modified = fullfill_subcmd(&config, &opts, &mut wishlist)?;
        }
    }

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

async fn fullfill_wishs(config: &Config, opts: &Opts, wishlist: &Wishlist) -> Result<()> {
    // May the work begin!
    warn!("apm is still in early alpha stage. DO NOT use me on production systems!");
    info!("Synchronizing package databases...");
    let downloader = executor::download::Downloader::new();
    let mut solver = solver::Solver::new();

    let localdb = db::LocalDb::new(opts.root.join("var/cache/apm/db"), config.repo.clone(), &config.arch);
    localdb.update(&downloader).await.context("Failed to refresh local package database")?;
    let dbs = localdb.get_all().context("Invalid local package database")?;
    for (baseurl, db_path) in dbs {
        solver::deb::read_deb_db(&db_path, solver.pool.as_mut(), &baseurl)?;
    }
    solver.finalize();

    info!("Resolving dependencies...");
    let res = solver.install(wishlist)?;
    // Translating result to list of actions
    let root = opts.root.clone();
    let machine_status = executor::MachineStatus::new(&root)?;
    let mut actions = machine_status.gen_actions(res.as_slice(), config.purge_on_remove);
    // Generate modifiers and apply them
    if opts.unpack_only {
        let modifier = executor::modifier::UnpackOnly::default();
        modifier.apply(&mut actions);
    }

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

fn fullfill_subcmd(config: &Config, opts: &Opts, wishlist: &mut Wishlist) -> Result<bool> {
    match opts.subcmd.as_ref().unwrap() {
        SubCmd::Add(add) => {
            wishlist.add(&add.name)?;
            success!("Package {} added to wishlist", &add.name);
            info!("To apply changes, re-run apm");
            Ok(true)
        }
        SubCmd::Rm(rm) => {
            wishlist.remove(&rm.name)?;
            success!("Package {} removed from wishlist", &rm.name);
            info!("To apply changes, re-run apm");
            Ok(true)
        },
        SubCmd::Search(search) => {
            let localdb = db::LocalDb::new(opts.root.join("var/cache/apm/db"), config.repo.clone(), &config.arch);
            let dbs = localdb.get_all().context("Invalid local package database")?;

            let mut solver = solver::Solver::new();
            for (baseurl, db_path) in dbs {
                solver::deb::read_deb_db(&db_path, solver.pool.as_mut(), &baseurl)?;
            }
            solver.finalize();

            let names = solver.pool.serach(&search.keyword)?;
            crate::WRITER.write_chunks("", &names)?;
            Ok(true)
        }
    }
}
