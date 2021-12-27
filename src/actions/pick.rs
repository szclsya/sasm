use super::{InstallRequest, UserRequest};
use crate::{
    db::LocalDb,
    debug,
    executor::MachineStatus,
    info, pool,
    types::{config::Opts, PkgSource, PkgVersion, VersionRequirement},
};

use anyhow::{bail, Context, Result};
use console::style;
use std::fmt;

pub fn pick(pkgname: &str, opts: &Opts, local_db: &LocalDb) -> Result<UserRequest> {
    debug!("Parsing deb dbs...");
    let dbs = local_db
        .get_all_package_db()
        .context("Invalid local package database")?;
    let local_repo = opts.root.join(crate::LOCAL_REPO_PATH);
    if !local_repo.is_dir() {
        std::fs::create_dir_all(&local_repo)?;
    }
    let pool = pool::source::create_pool(&dbs, &[local_repo])?;

    // To check if this package is installed already
    let ms = MachineStatus::new(&opts.root)?;
    let currrent_version = ms.pkgs.get(pkgname).map(|state| &state.version);
    // Get all versions
    let mut choices: Vec<(String, PkgVersion, bool, bool)> = Vec::new();
    if let Some(ids) = pool.get_pkgs_by_name(pkgname) {
        let mut first = true;
        for id in ids {
            let meta = pool.get_pkg_by_id(id).unwrap();
            let local = matches!(meta.source, PkgSource::Local(_));
            // Form version str for display
            let mut version_str = meta.version.to_string();
            let mut info_segments = Vec::new();
            if Some(&meta.version) == currrent_version {
                info_segments.push(style("current").bold().green().to_string());
            }
            if first {
                info_segments.push(style("latest").green().to_string());
            }
            if local {
                info_segments.push(style("local").cyan().to_string());
            }
            if !info_segments.is_empty() {
                version_str.push_str(&format!(" ({})", info_segments.join(", ")));
            }
            choices.push((version_str, meta.version.clone(), first, local));
            // Not the first anymore
            first = false;
        }
    } else {
        bail!("Package {} not found", style(pkgname).bold());
    }

    // Display them
    let choices_str: Vec<&str> = choices.iter().map(|ver| ver.0.as_str()).collect();
    info!("Please choose a version for {}:", style(pkgname).bold());
    let i = dialoguer::Select::with_theme(&OmaTheme::default())
        .items(&choices_str)
        .default(0)
        .interact()?;
    let (_, ver, latest, local) = &choices[i];
    let ver_req = if *latest {
        None
    } else {
        Some(VersionRequirement {
            lower_bond: Some((ver.clone(), true)),
            upper_bond: Some((ver.clone(), true)),
        })
    };
    // Generate UserRequest
    let req = UserRequest::Install(vec![InstallRequest {
        pkgname: pkgname.to_owned(),
        modify: true,
        install_recomm: false,
        ver_req,
        local: *local,
    }]);

    Ok(req)
}

#[derive(Default)]
pub struct OmaTheme;

impl dialoguer::theme::Theme for OmaTheme {
    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        let prefix = match active {
            true => (crate::cli::gen_prefix(&style("->").bold().to_string())),
            false => (crate::cli::gen_prefix("")),
        };

        write!(f, "{}{}", prefix, text)
    }
}
