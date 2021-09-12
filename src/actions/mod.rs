mod install_all;
mod search;
use search::search_deb_db;

use crate::{
    db::LocalDb,
    executor::{MachineStatus, PkgState},
    info,
    types::config::{Config, Opts, SubCmd, Wishlist},
};

use anyhow::{Context, Result};
use console::style;
use std::path::PathBuf;

/// bool in return type indicated whether the wishlist is altered
pub async fn fullfill_command(
    config: &Config,
    opts: &Opts,
    wishlist: &mut Wishlist,
) -> Result<bool> {
    let downloader = crate::executor::download::Downloader::new();
    let localdb = LocalDb::new(
        opts.root.join("var/cache/apm/db"),
        config.repo.clone(),
        &config.arch,
    );

    match &opts.subcmd {
        SubCmd::Install(add) => {
            // Modify wishlist
            for name in &add.names {
                wishlist.add(name)?;
            }
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Apply stuff
            install_all::install_all(&localdb, &downloader, wishlist, opts, config).await?;
            Ok(true)
        }
        SubCmd::Remove(rm) => {
            // Modify wishlist
            for name in &rm.names {
                wishlist.remove(name)?;
            }
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Apply stuff
            install_all::install_all(&localdb, &downloader, wishlist, opts, config).await?;
            Ok(true)
        }
        SubCmd::Refresh => {
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            Ok(false)
        }
        SubCmd::Execute | SubCmd::Upgrade => {
            info!("Refreshing local package databases...");
            localdb
                .update(&downloader)
                .await
                .context("Failed to refresh local package database")?;

            install_all::install_all(&localdb, &downloader, wishlist, opts, config).await?;
            Ok(true)
        }
        SubCmd::Search(search) => {
            let localdb = LocalDb::new(
                opts.root.join("var/cache/apm/db"),
                config.repo.clone(),
                &config.arch,
            );
            let dbs: Vec<PathBuf> = localdb
                .get_all()
                .context("Invalid local package database")?
                .into_iter()
                .map(|(_, path)| path)
                .collect();
            let machine_status = MachineStatus::new(&opts.root)?;

            for pkginfo in search_deb_db(&dbs, &search.keyword)? {
                // Construct prefix
                let prefix = match machine_status.pkgs.get(&pkginfo.name) {
                    Some(pkg) => match pkg.state {
                        PkgState::Installed => style("INSTALLED").green(),
                        PkgState::Unpacked => style("UNPACKED").yellow(),
                        _ => style("PACKAGE").dim(),
                    },
                    None => style("PACKAGE").dim(),
                }
                .to_string();
                // Construct pkg info line
                let mut pkg_info_line = style(&pkginfo.name).bold().to_string();
                pkg_info_line.push(' ');
                pkg_info_line.push_str(&style(pkginfo.version).green().to_string());
                if pkginfo.has_dbg_pkg {
                    pkg_info_line.push(' ');
                    pkg_info_line.push_str(&style("(debug symbols available)").dim().to_string())
                }
                crate::WRITER.writeln(&prefix, &pkg_info_line)?;
                crate::WRITER.writeln("", &pkginfo.description)?;
            }

            Ok(true)
        }
    }
}
