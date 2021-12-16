use super::{PkgInfo, search_rk_fast};
use crate::{db::LocalDb, debug, executor::MachineStatus, pool};

use anyhow::{Context, Result};
use console::style;
use flate2::read::GzDecoder;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
};
const READ_BUFFER_SIZE: usize = 1024 * 128;

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

/// Re-construct a package/path pair from position of interest
fn extract_content_line_from_poi(buffer: &[u8], pos: usize) -> Option<(&[u8], &[u8])> {
    let path_end = &buffer[pos..].iter().position(|c| c == &b' ')? + pos;
    let name_start = &buffer[path_end..].iter().position(|c| c == &b'/')? + path_end;
    let name_end = &buffer[name_start..].iter().position(|c| c == &b'\n')? + name_start;
    let path_start = &buffer[..pos].iter().rposition(|c| c == &b'\n')? + 1;

    Some((&buffer[(name_start + 1)..name_end], &buffer[path_start..path_end]))
}

// Given a filename or path, find package names that provide such file
pub fn package_name_provide_file(dbs: &[PathBuf], filename: &str) -> Result<HashMap<String, Vec<String>>> {
    // Construct search keyword based on deb Contents file format
    let keyword = format!("{} ", filename);

    let mut res: HashMap<String, Vec<String>> = HashMap::new();
    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    for db in dbs {
        let f = File::open(db)?;
        let f = GzDecoder::new(f);
        let mut bufreader = BufReader::new(f);
        let mut pkgs: Vec<(String, String)> = Vec::new();
        loop {
            let bytes_read = bufreader.read(&mut buffer[..READ_BUFFER_SIZE])?;
            if bytes_read < 1 {
                break;
            }
            // shorten the vector and move back the internal cursor
            buffer.truncate(bytes_read);
            let mut start = 0usize;
            bufreader.read_until(b'\n', &mut buffer)?;
            while let Some(pos) = search_rk_fast(&buffer[start..], keyword.as_bytes()) {
                // lazily parse this line
                if let Some(result) = extract_content_line_from_poi(&buffer, pos) {
                    let pkgname = std::str::from_utf8(result.0)?;
                    let mut path = std::str::from_utf8(result.1)?.to_string();
                    path.insert(0, '/');
                    pkgs.push((pkgname.to_string(), path));
                }
                start += pos + 1;
            }
            buffer.resize(READ_BUFFER_SIZE, 0);
        }

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
