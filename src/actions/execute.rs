use super::UserRequest;
use crate::{
    cli::{self, ask_confirm},
    db::LocalDb,
    debug,
    executor::{alpm, MachineStatus},
    info,
    pool::{self, PkgPool},
    solver::Solver,
    success,
    types::{
        config::{Blueprints, Config, Opts},
        PkgActionModifier,
    },
    utils::downloader::Downloader,
    warn,
};

use anyhow::{anyhow, bail, Context, Result};
use console::style;

// -> Result<UserCancelled?>
pub async fn execute(
    local_db: &LocalDb,
    downloader: &Downloader,
    blueprint: &mut Blueprints,
    opts: &Opts,
    config: &Config,
    request: UserRequest,
) -> Result<bool> {
    debug!("Parsing dpkg database...");
    let dbs = local_db.get_all_package_db().context("Invalid local package database!")?;
    let local_repo = opts.root.join(crate::LOCAL_REPO_PATH);
    if !local_repo.is_dir() {
        std::fs::create_dir_all(&local_repo)?;
    }
    let pool = pool::source::create_pool(&dbs, &[local_repo])?;

    debug!("Processing user request...");
    let root = &opts.root;
    let machine_status = MachineStatus::new(root).await?;
    process_user_request(request, pool.as_ref(), blueprint, &machine_status)?;

    debug!("Applying replaces according to package catalog...");
    apply_replaces(opts, pool.as_ref(), blueprint)?;

    info!("Resolving dependencies...");
    let solver = Solver::from(pool);
    let res = solver.install(blueprint)?;
    // Translating result to list of actions
    let actions = machine_status.gen_actions(res.as_slice());
    if actions.is_empty() {
        success!("There is nothing to do.");
        return Ok(false);
    }

    // There is something to do. Show it.
    info!("sasm will perform the following actions:");
    if opts.yes && opts.no_pager {
        actions.show();
    } else {
        actions.show_tables(opts.no_pager)?;
    }
    crate::WRITER.writeln("", "")?;
    actions.show_size_change();

    if ask_confirm(opts, "Proceed?")? {
        // Run it!
        Ok(false)
    } else {
        Ok(true)
    }
}

fn process_user_request(
    req: UserRequest,
    pool: &dyn PkgPool,
    blueprint: &mut Blueprints,
    ms: &MachineStatus,
) -> Result<()> {
    match req {
        UserRequest::Install(list) => {
            for install in list {
                // Check if this package actually exists
                if pool.get_pkgs_by_name(&install.pkgname).is_none() {
                    // Check if provides
                    if let Some(provider) = pool.find_provide(&install.pkgname, &install.ver_req) {
                        let e = anyhow!(
                            "Standalone package {} not found. However, {} provides package with this name. Add this package instead?",
                            style(&install.pkgname).bold(),
                            style(provider).bold()
                        );
                        return Err(e.context("Failed to add new package(s)."));
                    } else {
                        bail!("Failed to add new package: {}", install.pkgname);
                    }
                }

                // Add pkg to blueprint
                let add_res = blueprint.add(
                    &install.pkgname,
                    install.modify,
                    None,
                    install.ver_req,
                    install.local,
                );
                if let Err(e) = add_res {
                    warn!("Cannot add package {}: {e}", style(&install.pkgname).bold());
                }
            }
        }
        UserRequest::Remove(list) => {
            for (name, remove_recomm) in list {
                blueprint.remove(&name, remove_recomm)?;
            }
        }
        UserRequest::Upgrade => (),
    };

    Ok(())
}

fn apply_replaces(opts: &Opts, pool: &dyn PkgPool, blueprint: &mut Blueprints) -> Result<()> {
    // For every package in blueprint, check if they are replaced
    for pkg in blueprint.get_pkg_requests() {
        if let Some(replacement) = pool.find_replacement(&pkg.name, &pkg.version) {
            // Found a replacement!
            // If in user blueprint, ask if to replace it
            if blueprint.user_list_contains(&pkg.name) {
                if cli::ask_confirm(opts, &format!("Replace {} with {}?", pkg.name, replacement))? {
                    blueprint.remove(&pkg.name, true)?;
                    blueprint.add(&replacement, false, None, None, false)?;
                } else {
                    warn!("Package {} has been replaced by {}. Please update or edit vendor blueprint to use the new package.",
                          style(&pkg.name).bold(),
                          style(&replacement).bold());
                }
            }
        }
    }

    Ok(())
}
