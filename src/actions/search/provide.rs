use super::PkgInfo;
use crate::{db::LocalDb, debug, executor::MachineStatus, pool};

use anyhow::{Context, Result};
use console::style;
use flate2::read::GzDecoder;
use rayon::prelude::*;
use regex::Regex;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

pub fn show_provide_file(
    local_db: &LocalDb,
    filename: &str,
    machine_status: &MachineStatus,
) -> Result<()> {
    let content_dbs: Vec<PathBuf> = local_db
        .get_all_contents_db()
        .context("Cannot initialize local db for searching")?
        .into_iter()
        .map(|(_, path)| path)
        .collect();

    // Find a list of package names that provide the designated file
    debug!("Searching Contents databases...");
    let mut pkgnames = Vec::from_iter(package_name_provide_file(&content_dbs, filename)?);
    // Sort based on number of matched paths
    pkgnames.sort_by_key(|(_, paths)| paths.len());
    pkgnames.reverse();

    // Create a Solver so we can get more info    let mut solver = Solver::new();
    debug!("Constructing package pool...");
    let dbs = local_db
        .get_all_package_db()
        .context("Cannot initialize local db for searching")?;
    let pool = pool::source::create_pool(&dbs, &[])?;

    debug!("Generating detailed package info...");
    for (pkgname, paths) in pkgnames {
        if let Some(pkgs) = pool.get_pkgs_by_name(&pkgname) {
            // This is safe unless the pool is broken
            let latest_pkg_id = pkgs.get(0).unwrap();
            let latest_pkg = pool.get_pkg_by_id(*latest_pkg_id).unwrap();
            let provide_paths = paths.into_iter().map(|path| {
                format!("Provides: {}", style(path).bold())
            })
                .collect();
            // Prepare a PkgInfo
            let pkginfo = PkgInfo {
                pkg: latest_pkg,
                has_dbg_pkg: pool.has_dbg_pkg(*latest_pkg_id)?,
                additional_info: provide_paths,
            };
            pkginfo.show(machine_status)?;
        }
    }

    Ok(())
}

// Given a filename or path, find package names that provide such file
pub fn package_name_provide_file(dbs: &[PathBuf], filename: &str) -> Result<HashMap<String, Vec<String>>> {
    // Construct regex based on deb Contents file format
    let regex = if filename.starts_with('/') {
        // Absolute path, strip "/" to match Contents file format
        let path = filename.strip_prefix('/').unwrap();
        Regex::new(&format!(
            r"^(?P<path>{}) +[a-zA-Z0-9]+/(?P<pkgname>[-a-zA-Z0-9.+]+)$",
            path
        ))?
    } else {
        // Relative path, allow segments before filename
        Regex::new(&format!(
            r"^(?P<path>.*{}) +[a-zA-Z0-9]+/(?P<pkgname>[-a-zA-Z0-9.+]+)$",
            filename
        ))?
    };

    let mut res: HashMap<String, Vec<String>> = HashMap::new();
    for db in dbs {
        let f = File::open(db)?;
        let f = GzDecoder::new(f);
        let bufreader = BufReader::new(f);
        let pkgs: Vec<(String, String)> = bufreader
            .lines()
            .par_bridge()
            .filter_map(|line| match line {
                Ok(l) => {
                    if regex.is_match(&l) {
                        let captures = regex.captures(&l).unwrap();
                        let pkgname = captures.name("pkgname").unwrap().as_str().to_owned();
                        let mut path = captures.name("path").unwrap().as_str().to_owned();
                        // Add `/` to the front of path, because Contents file uses relative path
                        path.insert(0, '/');
                        Some((pkgname, path))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
        .collect();

        for (pkgname, path) in pkgs {
            if let Some(list) = res.get_mut(&pkgname) {
                list.push(path);
            } else {
                res.insert(pkgname, vec![path]);
            }
        }
    }

    Ok(res)
}
