/// Utilities to deal with deb package db
use super::{
    pool::PackagePool,
    types::PackageMeta,
    version::{PackageVersion, VersionRequirement},
    SolverError,
};
use anyhow::{format_err, Result};
use debcontrol::{BufParse, Streaming};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryFrom;

#[inline]
pub fn read_deb_db(
    db: &mut dyn std::io::Read,
    pool: &mut PackagePool,
    baseurl: &str,
) -> Result<(), SolverError> {
    let mut buf_parse = BufParse::new(db, 4096);
    while let Some(result) = buf_parse.try_next().unwrap() {
        match result {
            Streaming::Item(paragraph) => {
                let mut fields = HashMap::new();
                for field in paragraph.fields {
                    fields.insert(field.name, field.value);
                }
                pool.add(fields_to_packagemeta(&fields, baseurl)?);
            }
            Streaming::Incomplete => buf_parse.buffer().unwrap(),
        }
    }
    Ok(())
}

#[inline]
fn fields_to_packagemeta(f: &HashMap<&str, String>, baseurl: &str) -> anyhow::Result<PackageMeta> {
    // Generate real url
    let mut path = baseurl.to_string();
    path.push('/');
    path.push_str(
        f.get("Filename")
            .ok_or_else(|| format_err!("Package without filename"))?,
    );
    Ok(PackageMeta {
        name: f
            .get("Package")
            .ok_or_else(|| format_err!("Package without name"))?
            .to_string(),
        version: PackageVersion::try_from(
            f.get("Version")
                .ok_or_else(|| format_err!("Package without version"))?
                .as_str(),
        )?,
        depends: parse_pkg_list(f.get("Depends").unwrap_or(&String::new()))?,
        breaks: parse_pkg_list(f.get("Breaks").unwrap_or(&String::new()))?,
        conflicts: parse_pkg_list(f.get("Conflicts").unwrap_or(&String::new()))?,
        url: path,
    })
}

#[inline]
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
