use crate::solver::{
    pool::PackagePool,
    types::PackageMeta,
    version::{PackageVersion, VersionRequirement},
    SolverError,
};
/// Utilities to deal with deb package db
use anyhow::{format_err, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[inline]
pub fn read_deb_db(db_path: &Path, pool: &mut PackagePool) -> Result<(), SolverError> {
    let f = File::open(db_path).or_else(|_| {
        Err(SolverError::DatabaseInitError(format!(
            "Failed to open dpkg db {}",
            db_path.display()
        )))
    })?;
    let f = BufReader::with_capacity(50000, f);
    let mut field_buffer = HashMap::new();
    for (pos, l) in f.lines().enumerate() {
        let l = l.unwrap();
        if !l.is_empty() {
            let v: Vec<&str> = l.split(": ").collect();
            if v.len() == 2 {
                field_buffer.insert(v[0].to_string(), v[1].to_string());
            }
        } else {
            // parse all existing fields
            let pkg_meta = fields_to_packagemeta(&field_buffer).map_err(|e| {
                SolverError::DatabaseInitError(format!(
                    "Failed to parse section before line {}: {}",
                    pos,
                    e.to_string()
                ))
            })?;
            pool.add(pkg_meta);
            // new package section
            field_buffer.clear()
        }
    }
    Ok(())
}

#[inline]
fn fields_to_packagemeta(f: &HashMap<String, String>) -> anyhow::Result<PackageMeta> {
    Ok(PackageMeta {
        name: f
            .get("Package")
            .ok_or_else(|| format_err!("Package without name"))?
            .to_string(),
        version: PackageVersion::from(
            f.get("Version")
                .ok_or_else(|| format_err!("Package without version"))?,
        )?,
        depends: parse_pkg_list(f.get("Depends").unwrap_or(&String::new()))?,
        breaks: parse_pkg_list(f.get("Breaks").unwrap_or(&String::new()))?,
    })
}

fn parse_pkg_list(s: &str) -> anyhow::Result<Vec<(String, VersionRequirement)>> {
    lazy_static! {
        static ref PKG_PARTITION: Regex = Regex::new(
            r"^(?P<name>[A-Za-z0-9-.+]+)( \((?P<ver_req>[<>=]+ ?[A-Za-z0-9.\-:+~]+)\))?$"
        )
        .unwrap();
    }
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut res = Vec::new();
    let pkgs: Vec<&str> = s.split(", ").collect();
    for pkg in pkgs {
        let segments = PKG_PARTITION
            .captures(pkg)
            .ok_or_else(|| format_err!("Malformed version in depends/breaks: {}", pkg))?;
        // The regex should ensure name always exist
        let name = segments.name("name").unwrap().as_str().to_string();
        let ver_req = match segments.name("ver_req") {
            Some(s) => VersionRequirement::try_from(s.as_str())?,
            None => VersionRequirement::new(),
        };
        // Add to result
        res.push((name, ver_req));
    }

    Ok(res)
}
