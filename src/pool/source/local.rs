use crate::{
    debug,
    types::{PkgMeta, PkgSource, PkgVersion},
    utils::debcontrol::parse_pkg_list,
};

use anyhow::{bail, format_err, Context, Result};
use rayon::prelude::*;
use std::{collections::HashMap, ffi::OsStr, fs::File, io::prelude::*, path::Path};
use tar::Archive;
use xz2::read::XzDecoder;

pub fn read_debs_from_path(p: &Path) -> Result<Vec<PkgMeta>> {
    if !p.is_dir() {
        bail!(
            "Invalid local repository: {} is not a directory.",
            p.display()
        );
    }

    let mut deb_paths = Vec::new();
    for entry in std::fs::read_dir(p)? {
        let entry = entry?;
        let path = entry.path();
        debug!("Parsing local deb file {} ...", path.display());
        if !path.is_file() || path.extension() != Some(OsStr::new("deb")) {
            continue;
        }
        // Now we confirm it is a deb file. Add it to the process queue
        deb_paths.push(path);
    }

    deb_paths
        .par_iter()
        .map(|deb| read_control_from_deb(deb))
        .collect()
}

pub fn read_control_from_deb(p: &Path) -> Result<PkgMeta> {
    let mut archive = ar::Archive::new(
        File::open(p).context(format!("Failed to open deb file {} .", p.display()))?,
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
                    let res = parse_debcontrol(&res, p)?;
                    return Ok(res);
                }
            }
        }
    }
    bail!("Malformed deb file.")
}

fn parse_debcontrol(i: &str, p: &Path) -> Result<PkgMeta> {
    let paragraphs = match debcontrol::parse_str(i) {
        Ok(p) => p,
        Err(e) => bail!("Failed to parse control for deb: {} .", e),
    };
    let mut fields = HashMap::new();
    for p in paragraphs {
        for field in p.fields {
            fields.insert(field.name, field.value);
        }
    }

    parse_debcontrol_fields(fields, p)
}

fn parse_debcontrol_fields(mut f: HashMap<&str, String>, p: &Path) -> Result<PkgMeta> {
    Ok(PkgMeta {
        name: f
            .remove("Package")
            .ok_or_else(|| format_err!("deb control file does not contain the Package field."))?,
        section: f
            .remove("Section")
            .ok_or_else(|| format_err!("deb control file does not contain the Section field."))?,
        description: f
            .remove("Description")
            .ok_or_else(|| format_err!("deb control file does not contain the Description field."))?,
        version: PkgVersion::try_from(
            f.get("Version")
                .ok_or_else(|| format_err!("deb control file does not contain the Version field."))?
                .as_str(),
        )?,
        depends: parse_pkg_list(f.get("Depends").unwrap_or(&String::new()))?,
        breaks: parse_pkg_list(f.get("Breaks").unwrap_or(&String::new()))?,
        conflicts: parse_pkg_list(f.get("Conflicts").unwrap_or(&String::new()))?,
        recommends: match f.get("Recommends") {
            Some(recomm) => Some(parse_pkg_list(recomm)?),
            None => None,
        },
        suggests: match f.get("Suggests") {
            Some(suggests) => Some(parse_pkg_list(suggests)?),
            None => None,
        },
        provides: match f.get("Provides") {
            Some(provides) => Some(parse_pkg_list(provides)?),
            None => None,
        },
        replaces: match f.get("Replaces") {
            Some(replaces) => Some(parse_pkg_list(replaces)?),
            None => None,
        },
        // Installed-Size is in kilobytes, multiply by 1024 to convert it to bytes
        install_size: f
            .remove("Installed-Size")
            .ok_or_else(|| format_err!("deb control file does not contain the Installed-Size field."))?
            .as_str()
            .parse()
            .map(|kb: u64| 1024 * kb)?,
        essential: match f.get("Essential") {
            Some(word) => match word.as_str() {
                "yes" => true,
                "no" => false,
                invalid => bail!(
                    "deb control for {} contains invalid value for the Essential field (should be yes/no, got {}).",
                    p.display(),
                    invalid
                ),
            },
            None => false,
        },
        source: PkgSource::Local(p.to_owned()),
    })
}
