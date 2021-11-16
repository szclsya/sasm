use crate::{
    cli,
    db::LocalDb,
    debug,
    executor::{dpkg, modifier, MachineStatus},
    info,
    solver::{deb::read_deb_db, Solver},
    success,
    types::{
        config::{Blueprints, Config, IgnoreRules, Opts},
        PkgActionModifier,
    },
    utils::downloader::Downloader,
};

use anyhow::{Context, Result};
use dialoguer::Confirm;

#[inline]
pub async fn execute(
    local_db: &LocalDb,
    downloader: &Downloader,
    blueprint: &Blueprints,
    ignorerules: &IgnoreRules,
    opts: &Opts,
    config: &Config,
) -> Result<()> {
    let mut solver = Solver::new();

    let dbs = local_db
        .get_all()
        .context("Invalid local package database")?;
    debug!("Parsing deb debs...");
    for (baseurl, db_path) in dbs {
        read_deb_db(&db_path, solver.pool.as_mut(), &baseurl)?;
    }
    solver.finalize();

    info!("Resolving dependencies...");
    let res = solver.install(blueprint)?;
    // Translating result to list of actions
    let root = opts.root.clone();
    let machine_status = MachineStatus::new(&root)?;
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
            std::process::exit(2);
        }
    }

    Ok(())
}
