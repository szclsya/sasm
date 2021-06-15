mod cli;
mod solver;

use std::path::PathBuf;
fn main() {
    let arch_db = PathBuf::from(std::env::var("ARCH_DEB_DB").unwrap());
    let noarch_db = PathBuf::from(std::env::var("NOARCH_DEB_DB").unwrap());
    let args: Vec<String> = std::env::args().collect();

    println!("Importing db..");
    let solver = solver::Solver::from_dpkg_dbs(&[arch_db, noarch_db]).unwrap();
    println!("Solving..");
    let res = solver.install(&args[1..]).unwrap();
    println!("{:?}", res);
}
