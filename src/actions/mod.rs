mod search;
use search::search_deb_db;

use crate::{
    db::LocalDb,
    info, success,
    types::config::{Config, Opts, SubCmd, Wishlist},
};

use anyhow::{Context, Result};
use console::style;
use std::path::PathBuf;

/// bool in return type indicated whether the wishlist is altered
pub fn fullfill_command(config: &Config, opts: &Opts, wishlist: &mut Wishlist) -> Result<bool> {
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
        }
        SubCmd::Search(search) => {
            info!(
                "Searching local database for {}",
                style(&search.keyword).bold()
            );
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

            for pkginfo in search_deb_db(&dbs, &search.keyword)? {
                crate::WRITER.writeln(
                    &style("PKG").dim().to_string(),
                    &format!(
                        "{} {}",
                        &style(&pkginfo.name).bold().to_string(),
                        &style(pkginfo.version).green().to_string()
                    ),
                )?;
                crate::WRITER.writeln("", &pkginfo.description)?;
            }

            Ok(true)
        }
    }
}
