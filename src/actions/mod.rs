mod execute;
mod search;
use execute::execute;
use search::search_deb_db;

use crate::{
    db::LocalDb,
    executor::{MachineStatus, PkgState},
    info, success,
    types::config::{Blueprints, Config, Opts, SubCmd},
};

use anyhow::{Context, Result};
use console::style;
use std::path::PathBuf;

/// bool in return type indicated whether the blueprint is altered
pub async fn fullfill_command(
    config: &Config,
    opts: &Opts,
    blueprints: &mut Blueprints,
) -> Result<()> {
    let downloader = crate::executor::download::Downloader::new();
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
            execute(&localdb, &downloader, blueprints, opts, config).await?;
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
            execute(&localdb, &downloader, blueprints, opts, config).await?;
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

            execute(&localdb, &downloader, blueprints, opts, config).await?;
            Ok(())
        }
        SubCmd::Search(search) => {
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

            Ok(())
        }
    }
}
