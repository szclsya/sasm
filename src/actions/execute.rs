use super::UserRequest;
use crate::{
    cli,
    db::LocalDb,
    debug,
    executor::{dpkg, modifier, MachineStatus},
    info, pool,
    solver::Solver,
    success,
    types::{
        config::{Blueprints, Config, Opts},
        PkgActionModifier,
    },
    utils::downloader::Downloader,
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
    // Check if operating in alt-root mode
    let mut alt_root = false;
    if opts.root != std::path::Path::new("/") {
        info!(
            "Operating in external system root mode, Omakase will only unpack packages without configuration!"
        );
        alt_root = true;
    }

    // Load unsafe configs
    let unsafe_config = config.r#unsafe.clone().unwrap_or_default();

    debug!("Parsing dpkg database...");
    let dbs = local_db
        .get_all_package_db()
        .context("Invalid local package database!")?;
    let local_repo = opts.root.join(crate::LOCAL_REPO_PATH);
    if !local_repo.is_dir() {
        std::fs::create_dir_all(&local_repo)?;
    }
    let pool = pool::source::create_pool(&dbs, &[local_repo])?;

    debug!("Processing user request...");
    match request {
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
                        bail!("Failed to add new package(s).");
                    }
                }

                // Add pkg to blueprint
                blueprint.add(
                    &install.pkgname,
                    install.modify,
                    None,
                    install.ver_req,
                    install.local,
                )?;
                if !install.local && install.install_recomm {
                    let choices = match pool.get_pkgs_by_name(&install.pkgname) {
                        Some(pkgs) => pkgs,
                        None => bail!(
                            "Failed to add recommended packages for {} .",
                            &install.pkgname
                        ),
                    };
                    let choice = choices.get(0).unwrap();
                    let meta = pool.get_pkg_by_id(*choice).unwrap();
                    if let Some(recommends) = &meta.recommends {
                        for recommend in recommends {
                            blueprint.add(
                                &recommend.0,
                                false,
                                Some(&install.pkgname),
                                Some(recommend.1.clone()),
                                false,
                            )?;
                        }
                    }
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

    info!("Resolving dependencies...");
    let solver = Solver::from(pool);
    let res = solver.install(blueprint)?;
    // Translating result to list of actions
    let root = &opts.root;
    let machine_status = MachineStatus::new(root)?;
    let mut actions = machine_status.gen_actions(res.as_slice(), unsafe_config.purge_on_remove);
    if alt_root {
        let modifier = modifier::UnpackOnly::default();
        modifier.apply(&mut actions);
    }

    if actions.is_empty() {
        success!("There is nothing to do.");
        return Ok(false);
    }

    // There is something to do. Show it.
    info!("Omakase will perform the following actions:");
    if opts.yes && opts.no_pager {
        actions.show();
    } else {
        actions.show_tables(opts.no_pager)?;
    }
    crate::WRITER.writeln("", "")?;
    actions.show_size_change();

    // Additional confirmation if removing essential packages
    if actions.remove_essential() {
        if unsafe_config.allow_remove_essential {
            let prefix = style("DANGER").red().to_string();
            crate::WRITER.writeln(
                &prefix,
                "Some ESSENTIAL packages will be removed/purged. Are you REALLY sure?",
            )?;
            if cli::ask_confirm(opts, "Is this supposed to happen?")? {
                bail!("User cancelled operation.");
            }
        } else {
            bail!("Some ESSENTIAL packages will be removed. Aborting...")
        }
    }

    if cli::ask_confirm(opts, "Proceed?")? {
        // Run it!
        dpkg::execute_pkg_actions(actions, &opts.root, downloader, unsafe_config.unsafe_io).await?;
        Ok(false)
    } else {
        Ok(true)
    }
}
