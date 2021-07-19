mod cli;
mod executor;
mod repo;
mod solver;

use anyhow::{Context, Result};
use lazy_static::lazy_static;
use repo::RepoConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Deserialize)]
struct Config {
    arch: String,
    #[serde(default = "default_root")]
    root: String,
    purge_on_remove: bool,
    repo: HashMap<String, RepoConfig>,
    wishlist: HashMap<String, solver::VersionRequirement>,
}

#[inline]
fn default_root() -> String {
    "/".to_string()
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("ERROR: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("because: {}", cause));
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let config_path = PathBuf::from("/tmp/apm.toml");
    let mut config_file = File::open(&config_path).context("Failed to open config file")?;
    let mut data = String::new();
    config_file
        .read_to_string(&mut data)
        .context("Failed to read config file")?;
    let config: Config = toml::from_str(&data).context("Failed to parse config file")?;

    println!("Downloading and importing db..");
    let import_start = Instant::now();
    let mut solver = solver::Solver::new();

    let dbs = repo::get_dbs(&config.repo, &config.arch)?;
    for (baseurl, mut db) in dbs.into_iter() {
        solver::deb::read_deb_db(&mut db, &mut solver.pool, &baseurl)?;
    }
    solver.finalize();
    println!(
        "Reading deb db took {}s",
        import_start.elapsed().as_secs_f32()
    );

    println!("Solving..");
    let res = solver.install(&config.wishlist)?;

    // Translating result to list of actions
    let root = PathBuf::from(&config.root);
    let machine_status = executor::MachineStatus::new(&root)?;
    let actions = machine_status.gen_actions(res.as_slice(), config.purge_on_remove);
    for action in &actions {
        println!("{:?}", action);
    }

    // Run it!
    executor::dpkg::execute_pkg_actions(&actions, &PathBuf::from(&config.root))?;
    Ok(())
}
