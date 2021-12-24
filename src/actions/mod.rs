mod execute;
mod local;
mod provide;
mod search;
use execute::execute;
//use search::search_deb_db;

use crate::{
    db::LocalDb,
    executor::MachineStatus,
    info, success,
    types::{
        config::{Blueprints, Config, Opts, SubCmd},
        VersionRequirement,
    },
    utils::lock,
};

use anyhow::{Context, Result};
use std::path::PathBuf;

pub enum UserRequest {
    // Vec<(PkgName, ver_req, install_recomm, added_by, local)>
    Install(
        Vec<(
            String,
            Option<VersionRequirement>,
            bool,
            Option<String>,
            bool,
        )>,
    ),
    // Vec<(PkgName, remove_recomm)>
    Remove(Vec<(String, bool)>),
    Upgrade,
}

/// bool in return type indicated whether the blueprint is altered
pub async fn fullfill_command(
    config: &Config,
    opts: &Opts,
    blueprints: &mut Blueprints,
) -> Result<()> {
    let downloader = crate::utils::downloader::Downloader::new();
    // Directory that stores trusted public keys for repos
    let key_root = opts.root.join(crate::DB_KEY_PATH);
    let localdb = LocalDb::new(
        opts.root.join(crate::DB_CACHE_PATH),
        key_root,
        config.repo.clone(),
        &config.arch,
    );

    match &opts.subcmd {
        SubCmd::Install(add) => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            let names = if add.local {
                let paths: Vec<PathBuf> = add.names.iter().map(PathBuf::from).collect();
                local::add(&paths, &opts.root)?
            } else {
                add.names.clone()
            };
            let req = names
                .iter()
                .map(|pkgname| (pkgname.clone(), None, !add.no_recommends, None, add.local))
                .collect();
            let req = UserRequest::Install(req);
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Execute blueprint
            execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(())
        }
        SubCmd::Remove(rm) => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            // Prepare request
            let req: Vec<(String, bool)> = rm
                .names
                .iter()
                .map(|name| (name.clone(), rm.remove_recommends))
                .collect();
            let req = UserRequest::Remove(req);
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Apply stuff
            execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(())
        }
        SubCmd::Refresh => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            success!("Refresh complete");
            Ok(())
        }
        SubCmd::Execute => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            let req = UserRequest::Upgrade;
            info!("Refreshing local package databases...");
            localdb
                .update(&downloader)
                .await
                .context("Failed to refresh local package database")?;

            execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(())
        }
        SubCmd::Search(search) => {
            let machine_status = MachineStatus::new(&opts.root)?;
            search::search_deb_db(&localdb, &search.keyword, &machine_status)?;
            Ok(())
        }
        SubCmd::Provide(provide) => {
            let machine_status = MachineStatus::new(&opts.root)?;
            search::show_provide_file(
                &localdb,
                &machine_status,
                &provide.file,
                provide.first_only,
            )?;
            Ok(())
        }
        SubCmd::Clean(cleanconfig) => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            info!("Cleaning local package cache...");
            let pkg_cache_path = opts.root.join(crate::PKG_CACHE_PATH);
            if pkg_cache_path.is_dir() {
                std::fs::remove_dir_all(&pkg_cache_path)?;
                std::fs::create_dir_all(&pkg_cache_path)?;
            }

            info!("Cleaning local package repository...");
            let ms = MachineStatus::new(&opts.root)?;
            local::clean(&ms, &opts.root)?;

            if cleanconfig.all {
                info!("Cleaning local database cache...");
                let db_cache_path = opts.root.join(crate::DB_CACHE_PATH);
                if db_cache_path.is_dir() {
                    std::fs::remove_dir_all(&db_cache_path)?;
                    std::fs::create_dir_all(&db_cache_path)?;
                }
            }

            Ok(())
        }
    }
}
