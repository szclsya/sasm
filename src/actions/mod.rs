mod bench;
mod execute;
mod local;
mod pick;
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

#[derive(Debug)]
pub enum UserRequest {
    // Vec<(PkgName, ver_req, install_recomm, added_by, local)>
    Install(Vec<InstallRequest>),
    // Vec<(PkgName, remove_recomm)>
    Remove(Vec<(String, bool)>),
    Upgrade,
}

#[derive(Debug)]
pub struct InstallRequest {
    pkgname: String,
    install_recomm: bool,
    ver_req: Option<VersionRequirement>,
    local: bool,
    /// Whether modify existing entry
    modify: bool,
}

/// bool in return type indicated whether user cancelled operation
pub async fn fullfill_command(
    config: &Config,
    opts: &Opts,
    blueprints: &mut Blueprints,
) -> Result<bool> {
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
                local::add(opts, &paths)?
            } else {
                add.names.clone()
            };
            let req = names
                .iter()
                .map(|pkgname| InstallRequest {
                    pkgname: pkgname.to_owned(),
                    install_recomm: !add.no_recommends,
                    ver_req: None,
                    local: add.local,
                    modify: false,
                })
                .collect();
            let req = UserRequest::Install(req);
            // Update local db
            localdb.update(&downloader).await?;
            // Execute blueprint
            let cancelled = execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(cancelled)
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
            localdb.update(&downloader).await?;
            // Apply stuff
            let cancelled = execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(cancelled)
        }
        SubCmd::Pick(pick) => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            let req = pick::pick(&pick.name, blueprints, opts, &localdb)?;
            // Update local db
            localdb.update(&downloader).await?;
            // Apply stuff
            let cancelled = execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(cancelled)
        }
        SubCmd::Refresh => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            localdb.update(&downloader).await?;
            success!("Omakase has successfully refreshed local package metadata.");
            Ok(false)
        }
        SubCmd::Execute => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            let req = UserRequest::Upgrade;
            localdb
                .update(&downloader)
                .await
                .context("Failed to refresh local package metadata!")?;

            let exit = execute(&localdb, &downloader, blueprints, opts, config, req).await?;

            Ok(exit)
        }
        SubCmd::Search(search) => {
            let machine_status = MachineStatus::new(&opts.root)?;
            search::search_deb_db(&localdb, &search.keyword, &machine_status)?;
            Ok(false)
        }
        SubCmd::Provide(provide) => {
            let machine_status = MachineStatus::new(&opts.root)?;
            search::show_provide_file(
                &localdb,
                &machine_status,
                &provide.file,
                provide.first_only,
            )?;
            Ok(false)
        }
        SubCmd::Clean(cleanconfig) => {
            // This operation has side effects
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            info!("Purging local package metadata cache...");
            let pkg_cache_path = opts.root.join(crate::PKG_CACHE_PATH);
            if pkg_cache_path.is_dir() {
                std::fs::remove_dir_all(&pkg_cache_path)?;
                std::fs::create_dir_all(&pkg_cache_path)?;
            }

            info!("Purging local package cache...");
            let ms = MachineStatus::new(&opts.root)?;
            local::clean(&ms, &opts.root)?;

            if cleanconfig.all {
                info!("Purging local metadata cache...");
                let db_cache_path = opts.root.join(crate::DB_CACHE_PATH);
                if db_cache_path.is_dir() {
                    std::fs::remove_dir_all(&db_cache_path)?;
                    std::fs::create_dir_all(&db_cache_path)?;
                }
            }

            Ok(false)
        }
        SubCmd::Bench => {
            // This operation has side effects (refresh)
            lock::ensure_unlocked(&opts.root)?;
            lock::lock(&opts.root)?;

            bench::bench(opts, config, localdb, &downloader).await?;
            Ok(false)
        }
    }
}
