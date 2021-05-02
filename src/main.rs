mod apmsolver;
mod cli;

use std::env;
use std::fs;
use std::path::PathBuf;

const PACKAGE_FILE_PATH: &'static str = "/tmp/apm";

fn main() -> anyhow::Result<()> {
    // Scan apt's local directory
    let mut package_path: Vec<PathBuf> = Vec::new();
    for file in fs::read_dir(PACKAGE_FILE_PATH)? {
        let file = file?;
        let filename = file.file_name().into_string().unwrap();
        if filename.ends_with("_Packages") {
            package_path.push(PathBuf::from(PACKAGE_FILE_PATH).join(&filename));
        }
    }
    println!("Package dbs: {:#?}", &package_path);

    // Accept args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Supply argument (install/upgrade/remove/dist-upgrade)");
        return Ok(());
    }

    let operation = args[1].to_string();
    match operation.as_str() {
        "install" => {
            let to_install: Vec<String> = args[2..args.len()].to_vec();
            // Create solver instance
            let solver = apmsolver::ApmSolver::new(&package_path).unwrap();
            // Try some trivial solving
            let install_result = solver.install(&to_install).unwrap();
            println!("Installation detail: {:#?}", install_result);
        }
        "upgrade" => {
            let solver = apmsolver::ApmSolver::new(&package_path).unwrap();
            let upgrade_result = solver.upgrade().unwrap();
            println!("Upgrade detail: {:#?}", upgrade_result);
        }
        "dist-upgrade" => {
            let solver = apmsolver::ApmSolver::new(&package_path).unwrap();
            let upgrade_result = solver.dist_upgrade().unwrap();
            println!("Upgrade detail: {:#?}", upgrade_result);
        }
        "remove" => {
            let to_remove: Vec<String> = args[2..args.len()].to_vec();
            // Create solver instance
            let solver = apmsolver::ApmSolver::new(&package_path).unwrap();
            // Try some trivial solving
            let remove_result = solver.remove(&to_remove).unwrap();
            println!("Installation detail: {:#?}", remove_result);
        }
        _ => {
            println!("Unknown command: {}", &operation);
            return Ok(());
        }
    }
    Ok(())
}
