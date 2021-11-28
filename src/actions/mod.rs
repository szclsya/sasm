mod execute;
mod provide;
mod search;
use execute::execute;
//use search::search_deb_db;

use crate::{
    db::LocalDb,
    executor::MachineStatus,
    info, success,
    types::config::{Blueprints, Config, IgnoreRules, Opts, SubCmd},
};

use anyhow::{Context, Result};
use std::path::PathBuf;

/// bool in return type indicated whether the blueprint is altered
pub async fn fullfill_command(
    config: &Config,
    opts: &Opts,
    blueprints: &mut Blueprints,
    ignorerules: &mut IgnoreRules,
) -> Result<()> {
    let downloader = crate::utils::downloader::Downloader::new();
    // Directory that stores trusted public keys for repos
    let key_root = opts.root.join("etc/omakase/keys");
    let localdb = LocalDb::new(
        opts.root.join("var/cache/omakase/db"),
        key_root,
        config.repo.clone(),
        &config.arch,
    );

    match &opts.subcmd {
        SubCmd::Install(add) => {
            // Modify blueprint
            for name in &add.names {
                blueprints.add(name)?;
            }
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Execute blueprint
            execute(&localdb, &downloader, blueprints, ignorerules, opts, config).await?;
            Ok(())
        }
        SubCmd::Remove(rm) => {
            // Modify blueprint
            for name in &rm.names {
                blueprints.remove(name)?;
            }
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Apply stuff
            execute(&localdb, &downloader, blueprints, ignorerules, opts, config).await?;
            Ok(())
        }
        SubCmd::Refresh => {
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            success!("Refresh complete");
            Ok(())
        }
        SubCmd::Execute | SubCmd::Upgrade => {
            info!("Refreshing local package databases...");
            localdb
                .update(&downloader)
                .await
                .context("Failed to refresh local package database")?;

            execute(&localdb, &downloader, blueprints, ignorerules, opts, config).await?;

            Ok(())
        }
        SubCmd::Search(search) => {
            let dbs: Vec<PathBuf> = localdb
                .get_all_contents_db()
                .context("Invalid local package database")?
                .into_iter()
                .map(|(_, path)| path)
                .collect();
            let machine_status = MachineStatus::new(&opts.root)?;

            for pkginfo in search::search_deb_db(&dbs, &search.keyword)? {
                pkginfo.show(&machine_status)?;
            }

            Ok(())
        }
        SubCmd::Provide(provide) => {
            let dbs: Vec<PathBuf> = localdb
                .get_all_contents_db()
                .context("Invalid local package database")?
                .into_iter()
                .map(|(_, path)| path)
                .collect();

            for pkgname in search::search_file(&dbs, &provide.file)? {
                crate::WRITER.writeln("", &pkgname)?;
            }
            Ok(())
        }
    }
}
