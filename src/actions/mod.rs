mod install_all;
mod search;
use search::search_deb_db;

use crate::{
    db::LocalDb,
    executor::{MachineStatus, PkgState},
    info, success,
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

    // Default to InstallAll
    match opts.subcmd.as_ref().unwrap_or(&SubCmd::InstallAll) {
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
        }
        SubCmd::UpdateDb => {
            info!("Updating local package databases...");
            localdb.update(&downloader).await?;
            Ok(false)
        }
        SubCmd::InstallAll => {
            info!("Updating local package databases...");
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
