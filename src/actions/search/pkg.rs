use super::PkgInfo;
use crate::{
    db::LocalDb,
    solver::{read_deb_db, Solver},
    executor::MachineStatus,
};

use anyhow::{Context, Result};
use regex::Regex;

pub fn search_deb_db(local_db: &LocalDb, keyword: &str, machines_status: &MachineStatus) -> Result<()> {
    let mut solver = Solver::new();

    let dbs = local_db
        .get_all_package_db()
        .context("Cannot initialize local db for searching")?;
    for (baseurl, db_path) in dbs {
        read_deb_db(&db_path, solver.pool.as_mut(), &baseurl)?;
    }
    solver.finalize();

    let regex = Regex::new(keyword)?;
    let pkgs = search_deb_db_helper(&solver, &regex);

    // Display result
    for pkg in pkgs {
        pkg.show(machines_status)?;
    }

    Ok(())
} 

pub fn search_deb_db_helper<'a>(solver: &'a Solver, regex: &Regex) -> Vec<PkgInfo<'a>> {
    // Iterate through package names
    let mut res = Vec::new();
    for (name, versions) in solver.pool.pkgname_iter() {
        if regex.is_match(name) {
            let id = versions[0].0;
            let pkg = solver.pool.get_pkg_by_id(id).unwrap();
            let has_dbg_pkg = solver.pool.has_dbg_pkg(id).unwrap();

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
