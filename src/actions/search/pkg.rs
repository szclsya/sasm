use super::PkgInfo;
use crate::types::PkgVersion;

use anyhow::{bail, Result};
use debcontrol::{BufParse, Streaming};
use regex::Regex;
use std::{collections::HashMap, convert::TryFrom, fs::File, path::PathBuf};

pub fn search_deb_db(dbs: &[PathBuf], keyword: &str) -> Result<Vec<PkgInfo>> {
    let regex = Regex::new(keyword)?;

    let mut res: HashMap<String, PkgInfo> = HashMap::new();
    for db in dbs {
        let f = File::open(db)?;
        let mut buf_parse = BufParse::new(f, 4096);
        while let Some(result) = buf_parse.try_next().unwrap() {
            match result {
                Streaming::Item(paragraph) => {
                    let mut fields: HashMap<&str, String> = paragraph
                        .fields
                        .into_iter()
                        .map(|field| (field.name, field.value))
                        .collect();
                    if let Some(pkginfo) = match_pkg(&mut fields, &regex)? {
                        if !res.contains_key(pkginfo.name.as_str())
                            || res[pkginfo.name.as_str()].version < pkginfo.version
                        {
                            res.insert(pkginfo.name.clone(), pkginfo);
                        }
                    }
                }
                Streaming::Incomplete => buf_parse.buffer().unwrap(),
            }
        }
    }

    // Move dbg packages to a separate list
    let (mut res, dbg_pkgs): (HashMap<String, PkgInfo>, _) = res
        .into_iter()
        .partition(|(_, pkginfo)| pkginfo.section != "debug");
    // Add `has_dbg_pkg` property for corresponding packages
    for (name, pkginfo) in &mut res {
        let dbg_pkg_name = format!("{}-dbg", &name);
        if dbg_pkgs.contains_key(&dbg_pkg_name) {
            pkginfo.has_dbg_pkg = true;
        }
    }

    // Convert to a simple Vec
    let mut res: Vec<PkgInfo> = res.into_iter().map(|(_, pkginfo)| pkginfo).collect();
    res.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok(res)
}

fn match_pkg(fields: &mut HashMap<&str, String>, regex: &Regex) -> Result<Option<PkgInfo>> {
    let name = match fields.remove("Package") {
        Some(name) => name,
        None => bail!("Package without name"),
    };

    if !regex.is_match(&name) {
        return Ok(None);
    }

    let version = match fields.remove("Version") {
        Some(version) => PkgVersion::try_from(version.as_str())?,
        None => bail!("Package without Version"),
    };

    let section = match fields.remove("Section") {
        Some(section) => section,
        None => bail!("Package without Section"),
    };

    let description = match fields.remove("Description") {
        Some(name) => name,
        None => bail!("Package without Description"),
    };

    let res = PkgInfo {
        name,
        section,
        description,
        version,
        has_dbg_pkg: false,
    };

    Ok(Some(res))
}
