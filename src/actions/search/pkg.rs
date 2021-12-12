use super::PkgInfo;
use crate::{db::LocalDb, executor::MachineStatus, pool, pool::PkgPool};

use anyhow::{Context, Result};
use regex::Regex;

pub fn search_deb_db(
    local_db: &LocalDb,
    keyword: &str,
    machine_status: &MachineStatus,
) -> Result<()> {
    let dbs = local_db
        .get_all_package_db()
        .context("Cannot initialize local db for searching")?;
    let pool = pool::source::create_pool(&dbs, &[])?;

    let regex = Regex::new(keyword)?;
    let pkgs = search_deb_db_helper(pool.as_ref(), &regex);

    // Display result
    for pkg in pkgs {
        pkg.show(machine_status)?;
    }

    Ok(())
}

pub fn search_deb_db_helper<'a, P: ?Sized>(pool: &'a P, regex: &Regex) -> Vec<PkgInfo<'a>>
where
    P: PkgPool,
{
    // Iterate through package names
    let mut res = Vec::new();
    for (name, versions) in pool.pkgname_iter() {
        if regex.is_match(name) {
            let id = versions[0].0;
            let pkg = pool.get_pkg_by_id(id).unwrap();
            let has_dbg_pkg = pool.has_dbg_pkg(id).unwrap();

            // Construct PkgInfo, don't include debug packages
            if pkg.name.ends_with("-dbg") || pkg.section != "debug" {
                res.push(PkgInfo {
                    pkg,
                    has_dbg_pkg,
                    additional_info: None,
                })
            }
        }
    }

    res
}
