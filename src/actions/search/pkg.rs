use super::PkgInfo;
use crate::{db::LocalDb, executor::MachineStatus, pool, pool::PkgPool};

use anyhow::{Context, Result};
use std::{cmp::Reverse, collections::HashMap};

pub fn search_deb_db(
    local_db: &LocalDb,
    keyword: &str,
    machine_status: &MachineStatus,
) -> Result<()> {
    let dbs = local_db
        .get_all_package_db()
        .context("Failed to initialize local database for searching!")?;
    let pool = pool::source::create_pool(&dbs, &[])?;

    let mut pkgs = search_pkg_helper(pool.as_ref(), keyword);

    // Sort pkg in descending order based on relevance to keyword
    pkgs.sort_by_cached_key(|pkg| {
        Reverse((255.0 * strsim::jaro_winkler(&pkg.pkg.name, keyword)) as u8)
    });

    // Display result
    for pkg in pkgs {
        pkg.show(machine_status)?;
    }

    Ok(())
}

pub fn search_pkg_helper<'a, P: ?Sized>(pool: &'a P, keyword: &str) -> Vec<PkgInfo<'a>>
where
    P: PkgPool,
{
    // Iterate through package names
    let mut res = HashMap::new();
    for (name, versions) in pool.pkgname_iter() {
        if name.contains(keyword) {
            let id = versions[0].0;
            let pkg = pool.get_pkg_by_id(id).unwrap();
            let has_dbg_pkg = pool.has_dbg_pkg(id).unwrap();

            // Construct PkgInfo, don't include debug packages
            if !pkg.name.ends_with("-dbg") && pkg.section != "debug" {
                let pkginfo = PkgInfo {
                    pkg,
                    has_dbg_pkg,
                    additional_info: Vec::new(),
                };
                res.insert(name, pkginfo);
            }
        }
    }

    // Search package description
    for (id, meta) in pool.pkgid_iter() {
        if meta.description.contains(keyword) {
            if !res.contains_key(meta.name.as_str()) {
                let pkginfo = PkgInfo {
                    pkg: meta,
                    has_dbg_pkg: pool.has_dbg_pkg(id).unwrap(),
                    additional_info: Vec::new(),
                };
                res.insert(&meta.name, pkginfo);
            }
        }
    }

    let res = res.into_values().collect();
    res
}
