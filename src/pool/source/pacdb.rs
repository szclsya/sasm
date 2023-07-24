/// The pacman db reader
use crate::{
    debug,
    pool::PkgPool,
    types::{Checksum, PkgMeta, PkgSource, PkgVersion, VersionRequirement},
    utils::{downloader, pacparse},
    warn,
};
use anyhow::{anyhow, bail, Context, Result};
use rayon::prelude::*;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

use flate2::read::GzDecoder;
use tar::Archive;

pub fn import(db: &Path, pool: &mut dyn PkgPool, baseurl: &str) -> Result<()> {
    debug!("Importing package database from {}", db.display());
    let f = File::open(db)?;
    let gzipdecoder = GzDecoder::new(f);
    let mut tar = Archive::new(gzipdecoder);

    for file in tar.entries()? {
        let file = file.context("error reading file from db")?;
        let path = file.path()?.to_path_buf();
        let path_str = path.display().to_string();
        if path.ends_with("desc") {
            // Now we are talking!
            match parse_desc(file, &path_str) {
                Ok(pkgmeta) => {
                    pool.add(pkgmeta);
                }
                Err(e) => {
                    warn!("Failed to add {path_str} from {0}: {e}", db.display());
                }
            };
        }
    }
    Ok(())
}

fn parse_desc(mut f: impl Read, from: &str) -> Result<PkgMeta> {
    let mut content = String::new();
    f.read_to_string(&mut content).context("error reading desc file from db")?;
    let fields =
        pacparse::parse_str(&content).context(format!("error parsing desc from {from}"))?;
    let pkgmeta = fields_to_pkgmeta(fields).context(format!("error reading fields from {from}"))?;
    Ok(pkgmeta)
}

fn fields_to_pkgmeta(mut f: HashMap<String, String>) -> Result<PkgMeta> {
    // Get name first, for error reporting
    let name = f.remove("NAME").ok_or_else(|| anyhow!("bad metadata: missing NAME"))?;
    // Generate real url
    let path = f.remove("FILENAME").ok_or_else(|| anyhow!("bad metadata: missing FILENAME"))?;

    // Needed for source, so parse this first
    let download_size =
        f.remove("CSIZE").ok_or_else(|| anyhow!("bad metadata: missing CSIZE"))?.parse()?;
    Ok(PkgMeta {
        name: name.clone(),
        description: f.remove("DESC").ok_or_else(|| anyhow!("bad metadata for {name}"))?,
        version: PkgVersion::try_from(
            f.remove("VERSION").ok_or_else(|| anyhow!("bad metadata for {name}"))?.as_str(),
        )?,

        depends: get_pkg_list(&name, "DEPENDS", &mut f)?,
        optional: get_pkg_list(&name, "OPTDEPENDS", &mut f)?,
        conflicts: get_pkg_list(&name, "CONFLICTS", &mut f)?,
        install_size: f
            .remove("ISIZE")
            .ok_or_else(|| anyhow!("bad metadata: missing ISIZE"))?
            .parse()?,
        provides: get_pkg_list(&name, "PROVIDES", &mut f)?,
        replaces: get_pkg_list(&name, "REPLACES", &mut f)?,
        source: PkgSource::Http((path, download_size, {
            if let Some(hex) = f.get("SHA256SUM") {
                Checksum::from_sha256_str(&hex)?
            } else if let Some(hex) = f.get("SHA512SUM") {
                Checksum::from_sha512_str(&hex)?
            } else {
                bail!(
                        "Metadata for package {} does not contain the checksum field (SHA256 or SHA512).",
                        name
                    )
            }
        })),
    })
}

fn get_pkg_list(
    pkgname: &str,
    field_name: &str,
    f: &mut HashMap<String, String>,
) -> Result<Vec<(String, VersionRequirement, Option<String>)>> {
    let mut out = Vec::new();
    if let Some(values) = f.remove(field_name) {
        for (i, line) in values.lines().into_iter().enumerate() {
            // Parse the package line
            match pacparse::parse_package_requirement_line(&line) {
                Ok((_, (name, verreq, desc))) => out.push((name.to_owned(), verreq, desc)),
                Err(e) => {
                    warn!("bad package requirement when parsing {field_name}: {e}");
                    bail!("malformed package requirement for {pkgname} at line {i}");
                }
            }
        }
    }
    // It's fine to have nothing
    Ok(out)
}
