mod execute;
mod search;
use search::search_deb_db;
use execute::execute;

use crate::{
    db::LocalDb,
    executor::{MachineStatus, PkgState},
    info,success,
    types::config::{Config, Opts, SubCmd, Blueprint},
};

use anyhow::{Context, Result};
use console::style;
use std::path::PathBuf;

/// bool in return type indicated whether the blueprint is altered
pub async fn fullfill_command(
    config: &Config,
    opts: &Opts,
    blueprint: &mut Blueprint,
) -> Result<bool> {
    let downloader = crate::executor::download::Downloader::new();
    let localdb = LocalDb::new(
        opts.root.join("var/cache/apm/db"),
        config.repo.clone(),
        &config.arch,
    );

    match &opts.subcmd {
        SubCmd::Install(add) => {
            // Modify blueprint
            for name in &add.names {
                blueprint.add(name)?;
            }
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Execute blueprint
            execute(&localdb, &downloader, blueprint, opts, config).await?;
            Ok(true)
        }
        SubCmd::Remove(rm) => {
            // Modify blueprint
            for name in &rm.names {
                blueprint.remove(name)?;
            }
            // Update local db
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            // Apply stuff
            execute(&localdb, &downloader, blueprint, opts, config).await?;
            Ok(true)
        }
        SubCmd::Refresh => {
            info!("Refreshing local package databases...");
            localdb.update(&downloader).await?;
            success!("Refresh complete");
            Ok(false)
        }
        SubCmd::Execute | SubCmd::Upgrade => {
            info!("Refreshing local package databases...");
            localdb
                .update(&downloader)
                .await
                .context("Failed to refresh local package database")?;

            execute(&localdb, &downloader, blueprint, opts, config).await?;
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
