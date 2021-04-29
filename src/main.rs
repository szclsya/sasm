mod apmsolver;
mod cli;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let package_path: Vec<PathBuf> = vec![
        PathBuf::from("/tmp/apm/Packages-all"),
        PathBuf::from("/tmp/apm/Packages-amd64"),
    ];
    let to_install: Vec<String> = vec!["vim".to_string(), "emacs".to_string()];

    // Create solver instance
    let solver = apmsolver::ApmSolver::new(&package_path).unwrap();
    // Try some trivial solving
    let install_result = solver.install(&to_install).unwrap();
    println!("Installation detail: {:#?}", install_result);

    Ok(())
}
