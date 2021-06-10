use super::pool::PackagePool;
use super::types::*;
use anyhow::{bail, Result};

use varisat::ExtendFormula;
use varisat::{lit::Lit, solver::Solver};

fn generate(req: &Request, pool: &PackagePool) -> Result<()> {
    let mut solver = Solver::new();
    // Enroll requirements
    for r in &req.install {
        // Pick the latest version for each requested packages
        let choices = pool.get_ids(&r.0);
        let best_choice = choices.iter().max_by_key(|pkg| pkg.1.version.clone());
        if let Some(best) = best_choice {
            let l = Lit::from_dimacs(best.0 as isize);
            // And add it to the solver
            solver.add_clause(&[l]);
        } else {
            bail!("Cannot find package named {} in repository", r.0);
        }
    }

    // Enroll everything in the repo
    todo!()
}
