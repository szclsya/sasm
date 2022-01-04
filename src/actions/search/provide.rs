use super::PkgInfo;
use crate::{db::LocalDb, debug, executor::MachineStatus, pool};

use anyhow::{Context, Result};
use console::style;
use flate2::read::GzDecoder;
use regex::Regex;
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
};

const READ_BUFFER_SIZE: usize = 1000;

pub fn show_provide_file(
    local_db: &LocalDb,
    machine_status: &MachineStatus,
    filename: &str,
    bin: bool,
) -> Result<()> {
    let content_paths: Vec<PathBuf> = if bin {
        local_db.get_all_bincontents_db()
    } else {
        local_db.get_all_contents_db()
    }
    .context("Failed to initialize local database for searching!")?
    .into_iter()
    .map(|(_, path)| path)
    .collect();

    let mut content_dbs: Vec<Box<dyn Read>> = Vec::with_capacity(content_paths.len());
    for path in &content_paths {
        let f = File::open(path)?;
        if bin {
            // BinContents are not compressed
            content_dbs.push(Box::new(f));
        } else {
            // Contents are Gzip compressed
            let f = GzDecoder::new(f);
            content_dbs.push(Box::new(f));
        }
    }

    // Find a list of package names that provide the designated file
    debug!("Searching Contents metadata...");
    let mut pkgnames = Vec::from_iter(package_name_provide_file(content_dbs, filename)?);
    // Sort based on number of matched paths
    pkgnames.sort_by_key(|(_, paths)| Reverse(paths.len()));

    // Create a Solver so we can get more info    let mut solver = Solver::new();
    debug!("Constructing package pool...");
    let dbs = local_db.get_all_package_db()?;
    let pool = pool::source::create_pool(&dbs, &[])?;

    debug!("Generating detailed package information...");
    for (pkgname, paths) in pkgnames {
        if let Some(pkgs) = pool.get_pkgs_by_name(&pkgname) {
            // This is safe unless the pool is broken
            let latest_pkg_id = pkgs.get(0).unwrap();
            let latest_pkg = pool.get_pkg_by_id(*latest_pkg_id).unwrap();
            let provide_paths = paths
                .into_iter()
                .map(|path| format!("Provides: {}", style(path).bold()))
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
pub fn package_name_provide_file(
    dbs: Vec<Box<dyn Read>>,
    filename: &str,
) -> Result<HashMap<String, HashSet<String>>> {
    let regex =
        Regex::new(r"^(?P<path>[^\s]+) +[a-zA-Z0-9]+/(?P<pkgname>[-a-zA-Z0-9.+]+)$").unwrap();

    let mut res = HashMap::new();
    for db in dbs {
        let mut bufreader = BufReader::new(db);
        let mut buffer = vec![0u8; READ_BUFFER_SIZE];
        loop {
            let len = bufreader.read(&mut buffer)?;
            if len == 0 {
                // EOL reached
                break;
            }
            bufreader.read_until(b'\n', &mut buffer)?;
            scan_buffer(&buffer, &mut res, filename, &regex)?;
        }
    }

    Ok(res)
}

fn scan_buffer(
    buffer: &[u8],
    results: &mut HashMap<String, HashSet<String>>,
    filename: &str,
    regex: &Regex,
) -> Result<()> {
    let substring = format!("{} ", filename);
    for occurence in memchr::memmem::find_iter(buffer, &substring) {
        // Find line start
        let mut start = occurence;
        loop {
            if start == 0 || buffer[start - 1] == b'\n' {
                break;
            }
            start -= 1;
        }
        // Find line end
        let mut end = occurence;
        loop {
            if end == buffer.len() || buffer[end] == b'\n' {
                break;
            }
            end += 1;
        }

        let slice = &buffer[start..end];
        let line = std::str::from_utf8(slice)?;
        let captures = regex.captures(line).unwrap();
        let pkgname = captures.name("pkgname").unwrap().as_str().to_owned();
        let mut path = captures.name("path").unwrap().as_str().to_owned();
        // Add `/` to the front of path, because Contents file uses relative path
        if path.starts_with("./") {
            path.remove(0);
        } else {
            path.insert(0, '/');
        }

        if let Some(list) = results.get_mut(&pkgname) {
            list.insert(path);
        } else {
            let mut set = HashSet::new();
            set.insert(path);
            results.insert(pkgname, set);
        }
    }

    Ok(())
}
