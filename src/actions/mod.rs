mod execute;
use execute::execute;

use crate::{
    config::CachedRepoDb,
    config::{Blueprints, Config, Opts, SubCmd},
    executor::MachineStatus,
    info, success,
    types::VersionRequirement,
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
    let _key_root = opts.root.join(crate::DB_KEY_PATH);
    let localdb =
        CachedRepoDb::new(opts.root.join(crate::DB_CACHE_PATH), config.repo.clone(), &config.arch);

    match &opts.subcmd {
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
            let ms = MachineStatus::new(&opts.root).await?;

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
    }
}
