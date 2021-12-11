use crate::{
    types::{PkgVersion, VersionRequirement},
    utils::debcontrol::parse_pkg_list,
};

use anyhow::{bail, format_err, Context, Result};
use std::{collections::HashMap, fs::File, io::prelude::*, path::Path};
use tar::Archive;
use xz2::read::XzDecoder;

pub struct DebMeta {
    pub name: String,
    pub section: String,
    pub description: String,
    pub version: PkgVersion,
    pub depends: Vec<(String, VersionRequirement)>,
    pub breaks: Vec<(String, VersionRequirement)>,
    pub conflicts: Vec<(String, VersionRequirement)>,
    pub install_size: u64,
}

pub fn read_control_from_deb(path: &Path) -> Result<DebMeta> {
    let mut archive = ar::Archive::new(
        File::open(path).context(format!("Failed to open deb file at {}", path.display()))?,
    );
    while let Some(entry) = archive.next_entry() {
        let entry = entry?;
        let filename = std::str::from_utf8(entry.header().identifier())?;
        if filename == "control.tar.xz" {
            let xzdecoder = XzDecoder::new(entry);
            let mut tar = Archive::new(xzdecoder);
            for file in tar.entries()? {
                let mut file = file?;
                let path = file
                    .header()
                    .path()?
                    .to_str()
                    .unwrap_or_default()
                    .to_owned();
                if path == "./control" {
                    let mut res = String::new();
                    file.read_to_string(&mut res)?;
                    let res = parse_debcontrol(&res)?;
                    return Ok(res);
                }
            }
        }
    }
    bail!("Malformed deb file")
}

fn parse_debcontrol(i: &str) -> Result<DebMeta> {
    let paragraphs = match debcontrol::parse_str(i) {
        Ok(p) => p,
        Err(e) => bail!("Failed to parse control for deb: {}", e),
    };
    let mut fields = HashMap::new();
    for p in paragraphs {
        for field in p.fields {
            fields.insert(field.name, field.value);
        }
    }

    parse_debcontrol_fields(fields)
}

fn parse_debcontrol_fields(mut f: HashMap<&str, String>) -> Result<DebMeta> {
    Ok(DebMeta {
        name: f
            .remove("Package")
            .ok_or_else(|| format_err!("deb control without name"))?,
        section: f
            .remove("Section")
            .ok_or_else(|| format_err!("deb control without Section"))?,
        description: f
            .remove("Description")
            .ok_or_else(|| format_err!("deb control without Description"))?,
        version: PkgVersion::try_from(
            f.get("Version")
                .ok_or_else(|| format_err!("deb control without Version"))?
                .as_str(),
        )?,
        depends: parse_pkg_list(f.get("Depends").unwrap_or(&String::new()))?,
        breaks: parse_pkg_list(f.get("Breaks").unwrap_or(&String::new()))?,
        conflicts: parse_pkg_list(f.get("Conflicts").unwrap_or(&String::new()))?,
        install_size: f
            .remove("Installed-Size")
            .ok_or_else(|| format_err!("deb control without Installed-Size"))?
            .as_str()
            .parse()?,
    })
}
