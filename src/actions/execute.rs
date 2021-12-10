use super::UserRequest;
use crate::{
    cli,
    db::LocalDb,
    debug,
    executor::{dpkg, modifier, MachineStatus},
    info,
    solver::{read_deb_db, Solver},
    success,
    types::{
        config::{Blueprints, Config, IgnoreRules, Opts},
        PkgActionModifier,
    },
    utils::downloader::Downloader,
};

use anyhow::{bail, Context, Result};
use dialoguer::Confirm;

#[inline]
pub async fn execute(
    local_db: &LocalDb,
    downloader: &Downloader,
    blueprint: &mut Blueprints,
    ignorerules: &IgnoreRules,
    opts: &Opts,
    config: &Config,
    request: UserRequest,
) -> Result<()> {
    debug!("Parsing deb debs...");
    let mut solver = Solver::new();
    let dbs = local_db
        .get_all_package_db()
        .context("Invalid local package database")?;
    for (baseurl, db_path) in dbs {
        read_deb_db(&db_path, solver.pool.as_mut(), &baseurl)?;
    }
    solver.finalize();

    debug!("Processing user request...");
    match request {
        UserRequest::Install(list) => {
            for (name, instal_recomm) in list {
                // Add pkg to blueprint
                blueprint.add(&name, None, None)?;
                if instal_recomm {
                    let choices = match solver.pool.get_pkgs_by_name(&name) {
                        Some(pkgs) => pkgs,
                        None => bail!("Cannot add recommended packages for {}", &name),
                    };
                    let choice = choices.get(0).unwrap();
                    let meta = solver.pool.get_pkg_by_id(*choice).unwrap();
                    if let Some(recommends) = &meta.recommends {
                        for recommend in recommends {
                            blueprint.add(&recommend.0, Some(&name), Some(recommend.1.clone()))?;
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
    let res = solver.install(blueprint)?;
    // Translating result to list of actions
    let root = &opts.root;
    let machine_status = MachineStatus::new(root)?;
    let mut actions = machine_status.gen_actions(res.as_slice(), config.purge_on_remove);
    // Generate modifiers and apply them
    let ignore_modifier = modifier::IgnorePkgs::new(ignorerules)?;
    ignore_modifier.apply(&mut actions);
    if opts.unpack_only {
        let modifier = modifier::UnpackOnly::default();
        modifier.apply(&mut actions);
    }

    if actions.is_empty() {
        success!("There's nothing to do, all wishes has been fulfilled!");
    } else {
        info!("These following actions will be performed:");
        actions.show();
        crate::WRITER.writeln("", "")?;
        actions.show_size_change();
        if Confirm::new()
            .with_prompt(format!("{}{}", cli::gen_prefix(""), "Proceed?"))
            .interact()?
        {
            // Run it!
            dpkg::execute_pkg_actions(actions, &opts.root, downloader).await?;
        } else {
            crate::utils::lock::unlock(&opts.root)?;
            std::process::exit(2);
        }
    }

    Ok(())
}
