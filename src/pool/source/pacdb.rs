/// The pacman db reader
use crate::{
    debug, warn,
    pool::PkgPool,
    types::{Checksum, PkgMeta, PkgSource, PkgVersion, VersionRequirement},
    utils::{downloader, pacparse},
};
use anyhow::{bail, format_err, Context, Result};
use rayon::prelude::*;
use std::{collections::HashMap, fs::File, io::Read, path::Path};

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
                Ok(pkgmeta) => { pool.add(pkgmeta); },
                Err(e) => { warn!("Failed to add {path_str} from {0}: {e}", db.display()); },
            };
        }
    }
    Ok(())
}

fn parse_desc(mut f: impl Read, from: &str) -> Result<PkgMeta> {
    let mut content = String::new();
    f.read_to_string(&mut content).context("error reading desc file from db")?;
    let fields = pacparse::parse_str(&content)
                .context(format!("error parsing desc from {from}"))?;
    let pkgmeta = fields_to_pkgmeta(fields)
                .context(format!("error reading fields from {from}"))?;
    Ok(pkgmeta)
}

fn fields_to_pkgmeta(mut f: HashMap<String, Vec<String>>) -> Result<PkgMeta> {
    // Get name first, for error reporting
    let name = get_first_or_complain("NAME", &mut f)
        .map_err(|e| format_err!("bad metadata: NAME missing ({e})"))?;
    // Generate real url
    let path = get_first_or_complain("FILENAME", &mut f)
        .map_err(|e| format_err!("bad metadata: FILENAME missing ({e})"))?;

    // Needed for source, so parse this first
    let download_size = get_first_or_complain("CSIZE", &mut f)?.parse()?;
    Ok(PkgMeta {
        name: name.clone(),
        description: get_first_or_complain("DESC", &mut f)
            .map_err(|e| format_err!("bad metadata for {name}: {e}"))?,
        version: PkgVersion::try_from(
            get_first_or_complain("VERSION", &mut f)
                .map_err(|e| format_err!("bad metadata for {name}: {e}"))?
                .as_str(),
        )?,

        depends: get_pkg_list(&name, "DEPENDS", &mut f)?,
        optional: get_pkg_list(&name, "OPTDEPENDS", &mut f)?,
        conflicts: get_pkg_list(&name, "CONFLICTS", &mut f)?,
        install_size: get_first_or_complain("ISIZE", &mut f)?.parse()?,
        download_size,
        provides: get_pkg_list(&name, "PROVIDES", &mut f)?,
        replaces: get_pkg_list(&name, "REPLACES", &mut f)?,
        source: PkgSource::Http((path, download_size, {
            if let Some(hex) = f.get("SHA256SUM") {
                Checksum::from_sha256_str(&hex[0])?
            } else if let Some(hex) = f.get("SHA512SUM") {
                Checksum::from_sha512_str(&hex[0])?
            } else {
                bail!(
                        "Metadata for package {} does not contain the checksum field (SHA256 or SHA512).",
                        name
                    )
            }
        })),
    })
}

fn get_first_or_complain(name: &str, f: &mut HashMap<String, Vec<String>>) -> Result<String> {
    if let Some(mut values) = f.remove(name) {
        if values.len() == 1 {
            Ok(values.remove(0))
        } else {
            bail!("expect 1 value for {name}, found {}", values.len())
        }
    } else {
        bail!("field {name} not found")
    }
}

fn get_pkg_list(
    pkgname: &str,
    field_name: &str,
    f: &mut HashMap<String, Vec<String>>,
) -> Result<Vec<(String, VersionRequirement, Option<String>)>> {
    let mut out = Vec::new();
    if let Some(values) = f.remove(field_name) {
        for (i, line) in values.into_iter().enumerate() {
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
