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
        config::{Blueprints, Config, IgnorePkg, IgnoreRules, Opts, SubCmd},
        VersionRequirement,
    },
};

use anyhow::{Context, Result};
use console::style;
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
    ignorerules: &mut IgnoreRules,
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
            let names = if add.local {
                let paths: Vec<PathBuf> =
                    add.names.iter().map(|path| PathBuf::from(path)).collect();
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
            execute(
                &localdb,
                &downloader,
                blueprints,
                ignorerules,
                opts,
                config,
                req,
            )
            .await?;
            Ok(())
        }
        SubCmd::Remove(rm) => {
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
            execute(
                &localdb,
                &downloader,
                blueprints,
                ignorerules,
                opts,
                config,
                req,
            )
            .await?;
            Ok(())
        }
        SubCmd::Ignore(ignore) => {
            match ignore {
                IgnorePkg::Add(flags) => {
                    for rule in &flags.rules {
                        info!("Adding {} to IgnoreRules...", style(rule).bold());
                        ignorerules.add(rule.to_owned())?;
                    }
                    success!("Rules have been added");
                }
                IgnorePkg::Remove(flags) => {
                    for rule in &flags.rules {
                        info!("Removing {} from IgnoreRules...", style(rule).bold());
                        ignorerules.remove(rule)?;
                    }
                    success!("Rules have been added");
                }
                IgnorePkg::Show => {
                    for (info, rules) in ignorerules.gen_human_readable()? {
                        info!("Rules in {}", style(info).bold());
                        for rule in rules {
                            crate::WRITER.writeln("", &rule)?;
                        }
                    }
                }
            }
            Ok(())
        }
        SubCmd::Refresh => {
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            success!("Refresh complete");
            Ok(())
        }
        SubCmd::Execute | SubCmd::Upgrade => {
            let req = UserRequest::Upgrade;
            info!("Refreshing local package databases...");
            localdb
                .update(&downloader)
                .await
                .context("Failed to refresh local package database")?;

            execute(
                &localdb,
                &downloader,
                blueprints,
                ignorerules,
                opts,
                config,
                req,
            )
            .await?;

            Ok(())
        }
        SubCmd::Search(search) => {
            let machine_status = MachineStatus::new(&opts.root)?;
            search::search_deb_db(&localdb, &search.keyword, &machine_status)?;
            Ok(())
        }
        SubCmd::Provide(provide) => {
            let machine_status = MachineStatus::new(&opts.root)?;
            search::show_provide_file(&localdb, &provide.file, &machine_status)?;
            Ok(())
        }
        SubCmd::Clean(cleanconfig) => {
            info!("Cleaning local package cache...");
            let pkg_cache_path = opts.root.join(crate::PKG_CACHE_PATH);
            std::fs::remove_dir_all(&pkg_cache_path)?;
            std::fs::create_dir_all(&pkg_cache_path)?;

            info!("Cleaning local package repository...");
            let ms = MachineStatus::new(&opts.root)?;
            local::clean(&ms, &opts.root)?;

            if cleanconfig.all {
                info!("Cleaning local database cache...");
                let db_cache_path = opts.root.join(crate::DB_CACHE_PATH);
                std::fs::remove_dir_all(&db_cache_path)?;
                std::fs::create_dir_all(&db_cache_path)?;
            }
            Ok(())
        }
    }
}
