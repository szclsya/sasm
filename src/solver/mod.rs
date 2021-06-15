mod pool;
mod sat;
mod types;
mod version;

use anyhow::format_err;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::solver::types::PackageMeta;
use crate::solver::version::PackageVersion;

use self::types::VersionRequirement;

#[derive(Clone, Debug)]
pub enum SolverError {
    Unsolvable(String),
    DatabaseInitError(String),
    InternalError(String),
}

impl From<anyhow::Error> for SolverError {
    fn from(e: anyhow::Error) -> Self {
        SolverError::InternalError(e.to_string())
    }
}

pub struct Solver {
    pool: pool::PackagePool,
}

impl Solver {
    pub fn from_dpkg_db(db_path: &Path) -> Result<Self, SolverError> {
        let mut pool = pool::PackagePool::new();

        let f = File::open(db_path).or(Err(SolverError::DatabaseInitError(format!(
            "Failed to open dpkg db {}",
            db_path.display()
        ))))?;
        let f = BufReader::new(f);
        let mut field_buffer = HashMap::new();
        for l in f.lines() {
            let l = l.unwrap();
            if !l.is_empty() {
                let v: Vec<&str> = l.split(": ").collect();
                if v.len() == 2 {
                    field_buffer.insert(v[0].to_string(), v[1].to_string());
                }
            } else {
                // parse all existing fields
                let pkg_meta = fields_to_packagemeta(&field_buffer)?;
                pool.add(pkg_meta);
                // new package section
                field_buffer.clear()
            }
        }

        Ok(Solver {
            pool
        })
    }

    pub fn install(&self, to_install: &[String]) -> Result<Vec<types::PackageMeta>, SolverError> {
        todo!()
    }
}

fn fields_to_packagemeta(f: &HashMap<String, String>) -> Result<types::PackageMeta, SolverError> {
    let bad_db_err = SolverError::DatabaseInitError("Malformed deb repository".to_string());

    Ok(PackageMeta {
        name: f.get("Package").ok_or(bad_db_err.clone())?.to_string(),
        version: version::PackageVersion::from(f.get("Version").ok_or(bad_db_err.clone())?)
            .map_err(|e| SolverError::DatabaseInitError(e.to_string()))?,
        depends: parse_pkg_list(f.get("Depends").unwrap_or(&String::new()))?,
        breaks: parse_pkg_list(f.get("Breaks").unwrap_or(&String::new()))?,
    })
}

fn parse_pkg_list(s: &str) -> anyhow::Result<Vec<(String, VersionRequirement)>> {
    lazy_static! {
        static ref PKG_PARTITION: Regex = Regex::new(
            r"^(?P<name>[A-Za-z0-9-+]+)( \((?P<req_type>[<>=]*) (?P<req_ver>[A-Za-z0-9.\-]+\)))?$"
        )
        .unwrap();
    }
    let mut res = Vec::new();
    let pkgs: Vec<&str> = s.split(", ").collect();
    for pkg in pkgs {
        let segments = PKG_PARTITION.captures(pkg).ok_or(format_err!("Malformed version in depends/breaks"))?;
        // The regex should ensure name always exist
        let name = segments.name("name").unwrap().as_str().to_string();
        let mut ver_req = VersionRequirement {
            upper_bond: None,
            lower_bond: None,
        };
        if let Some(req_type) = segments.name("req_type") {
            // The regex should ensure req_name and req_type must coexist
            let ver = PackageVersion::from(segments.name("req_ver").unwrap().as_str())?;
            match req_type.as_str() {
                ">" => { ver_req.lower_bond = Some((ver, false)); },
                ">=" => { ver_req.lower_bond = Some((ver, true)); },
                "<" => { ver_req.upper_bond = Some((ver, false));},
                "<=" => { ver_req.upper_bond = Some((ver, true)); },
                _ => {}
            }
        }
        // Add to result
        res.push((name, ver_req));
    }

    Ok(res)
}
