mod cli;
mod solver;

use std::path::PathBuf;
use std::time::Instant;
fn main() {
    let arch_db = PathBuf::from(std::env::var("ARCH_DEB_DB").unwrap());
    let noarch_db = PathBuf::from(std::env::var("NOARCH_DEB_DB").unwrap());
    let args: Vec<String> = std::env::args().collect();

    println!("Importing db..");
    let import_start = Instant::now();
    let solver = solver::Solver::from_dpkg_dbs(&[arch_db, noarch_db]).unwrap();
    println!(
        "Reading deb db took {}s",
        import_start.elapsed().as_secs_f32()
    );

    println!("Solving..");
    let solve_start = Instant::now();
    let res = solver.install(&args[1..]).unwrap();
    for pkg in res.iter() {
        print!("{} {}, ", pkg.0, pkg.1);
    }
    println!("Total package install: {}", res.len());
    println!(
        "Dependency calculation took {}s",
        solve_start.elapsed().as_secs_f32()
    );
}
