use super::{InstallRequest, UserRequest};
use crate::{
    db::LocalDb,
    error,
    executor::MachineStatus,
    info, msg, pool,
    types::{config::Blueprints, config::Opts, PkgSource, PkgVersion, VersionRequirement},
};

use anyhow::{bail, Context, Result};
use console::style;

pub fn pick(
    pkgname: &str,
    blueprints: &Blueprints,
    opts: &Opts,
    local_db: &LocalDb,
) -> Result<UserRequest> {
    // Don't allow picking if the target is in the vendor blueprint
    if let Some(path) = blueprints.vendor_list_contains(pkgname) {
        error!(
            "Cannot pick version for {} because it is in vendor blueprint {}",
            style(pkgname).bold(),
            style(path.display()).bold()
        );
        msg!("Vendor blueprints cannot be modified by Omakase for safety reason. If you really wish to pick a version for this package, edit this file directly");
        bail!("Cannot pick version for {}", style(pkgname).bold())
    }

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
    let i = dialoguer::Select::with_theme(&crate::cli::OmaTheme::default())
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
    let req = vec![InstallRequest {
        pkgname: pkgname.to_owned(),
        modify: true,
        install_recomm: false,
        ver_req,
        local: *local,
    }];

    Ok(UserRequest::Install((req, false)))
}
