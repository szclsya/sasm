use super::{pool::PkgPool, solve, sort::sort_pkgs_to_cycles};

use anyhow::Result;
use std::collections::HashSet;
use varisat::{lit::Lit, ExtendFormula, Solver};

/// Attempt to use latest possible version of packages via forcing the solver to choose better versions
/// of packages via banning older versions via solver assume
pub fn upgrade(pool: &dyn PkgPool, res: &mut Vec<usize>, solver: &mut Solver) -> Result<()> {
    let mut assumes = Vec::new();
    let mut cant_update: HashSet<String> = HashSet::new();
    loop {
        let updates = gen_update_assume(pool, res);
        if !updates.is_empty() {
            let names: HashSet<String> = updates.iter().map(|(name, _)| name.to_string()).collect();
            // If the update list only contains packages that can't be upgraded, stop
            if cant_update == names {
                break;
            }
            for update in updates {
                let mut new_assumes = assumes.clone();
                new_assumes.append(&mut update.1.clone());
                solver.assume(&new_assumes);
                if solver.solve().unwrap() {
                    *res = solve(solver)?;
                    assumes = new_assumes;
                } else {
                    cant_update.insert(update.0);
                }
            }
        } else {
            break;
        }
    }

    Ok(())
}

/// Construct a subset list of packages that only contains equal version of existing packages
/// So that no older packages are included when upgrading packages
pub fn reduce(pool: &dyn PkgPool, res: &mut Vec<usize>, to_install: &[usize]) -> Result<()> {
    // Generate reduced formula
    let mut formula = pool.gen_formula(Some(res));
    for pkgid in to_install {
        formula.add_clause(&[Lit::from_dimacs(*pkgid as isize)]);
    }

    let mut solver = Solver::new();
    solver.add_formula(&formula);
    // Initial solve
    *res = solve(&mut solver)?;

    // Try remove this package from the list of cycles
    let cycles = sort_pkgs_to_cycles(pool, res)?;
    let mut assumes = Vec::new();
    cycles.iter().for_each(|cycle| {
        let mut no_ids: Vec<Lit> = cycle
            .iter()
            .map(|id| !Lit::from_dimacs(*id as isize))
            .collect();
        let mut new_assume = assumes.clone();
        new_assume.append(&mut no_ids);
        solver.assume(&new_assume);
        if solver.solve().unwrap() {
            // If can be solved without the cycle, it should be safe to remove it
            assumes = new_assume;
        }
    });

    solver.assume(&assumes);
    *res = solve(&mut solver).unwrap();
    // Reset solver
    Ok(())
}

/// Generate a list of Lit of all older packages
/// The idea is that with these assumptions, the SAT solver must choose more up-to-date
///   packages, or give Unsolvable
pub fn gen_update_assume(pool: &dyn PkgPool, ids: &[usize]) -> Vec<(String, Vec<Lit>)> {
    let mut res = Vec::new();
    for id in ids {
        if !is_best(pool, *id).unwrap() {
            let mut assume = Vec::new();
            // Find all newer versions of this package
            let name = &pool.get_pkg_by_id(*id).unwrap().name;
            let pkgids: Vec<usize> = pool.get_pkgs_by_name(name).unwrap();

            let mut reached = false;
            for pkgid in pkgids {
                if pkgid == *id {
                    reached = true
                }
                if reached {
                    reached = true;
                    let lit = !Lit::from_dimacs(pkgid as isize);
                    assume.push(lit);
                }
            }
            res.push((name.to_string(), assume));
        }
    }
    res
}

#[inline]
pub fn is_best(pool: &dyn PkgPool, id: usize) -> Option<bool> {
    let name = &pool.get_pkg_by_id(id)?.name;
    let ids = pool.get_pkgs_by_name(name)?;
    if ids[0] != id {
        Some(false)
    } else {
        Some(true)
    }
}
