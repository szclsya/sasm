use crate::types::PkgVersion;

use anyhow::{bail, Result};
use debcontrol::{BufParse, Streaming};
use regex::Regex;
use std::{collections::HashMap, convert::TryFrom, fs::File, path::PathBuf};

pub struct PkgInfo {
    pub name: String,
    pub description: String,
    pub version: PkgVersion,
}

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
                            || res[pkginfo.name.as_str()].version < pkginfo.version {
                            res.insert(pkginfo.name.clone(), pkginfo);
                        }
                    }
                }
                Streaming::Incomplete => buf_parse.buffer().unwrap(),
            }
        }
    }

    // Convert to a simple Vec
    let res: Vec<PkgInfo> = res.into_iter().map(|(_, pkginfo)| pkginfo).collect();
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

    let description = match fields.remove("Description") {
        Some(name) => name,
        None => bail!("Package without Description"),
    };

    let res = PkgInfo {
        name,
        description,
        version,
    };

    Ok(Some(res))
}
