use super::{ pool::PackagePool, solve };

use anyhow::Result;
use varisat::{lit::Lit, ExtendFormula, Solver};

/// Improve the results of initial dependency calculation
pub fn improve(pool: &PackagePool, res: &mut Vec<usize>, solver: &mut Solver) -> Result<()> {
    let mut assumes: Vec<Lit> = Vec::new();
    upgrade(pool, res, solver, &mut assumes)?;
    reduce(pool, res, solver, &mut assumes)?;
    Ok(())
}

/// Construct a subset list of packages that only contains equal or newer versions of existing packages
/// So that no new packages are included when upgrading packages
pub fn reduced_upgrade(pool: &PackagePool, res: &mut Vec<usize>, to_install: &[usize]) -> Result<()> {
    // Generate reduced formula
    let mut ids = Vec::new();
    for pkg in res.iter() {
        let pkgmeta = pool.id_to_pkg(*pkg).unwrap();
        let pkgs_with_name = pool.pkg_name_to_ids(&pkgmeta.name).unwrap();
        for (pkgid, _) in pkgs_with_name {
            ids.push(pkgid);
            if pkgid == *pkg {
                break;
            }
        }
    }
    // Formulate formula
    let mut formula = pool.gen_subset_formula(&ids);
    for pkgid in to_install {
        formula.add_clause(&[Lit::from_dimacs(*pkgid as isize)]);
    }

    let mut solver = Solver::new();
    solver.add_formula(&formula);
    // Initial solve
    *res = solve(&mut solver)?;
    upgrade(pool, res, &mut solver, &mut Vec::new())?;
    Ok(())
}

/// Attempt to use latest possible version of packages via forcing the solver to choose better versions
/// of packages via banning older versions via solver assume
fn upgrade(pool: &PackagePool, res: &mut Vec<usize>, solver: &mut Solver, assumes: &mut Vec<Lit>) -> Result<()> {
    loop {
        let mut updates = gen_update_assume(pool, res);
        if !updates.is_empty() {
            let mut new_assumes = assumes.clone();
            new_assumes.append(&mut updates);
            solver.assume(&new_assumes);
            if solver.solve().unwrap() {
                *res = solve(solver)?;
                *assumes = new_assumes;
            } else {
                // Cannot update any further
                break;
            }
        } else {
            break;
        }
    }
    // Reset solver
    solver.assume(&[]);

    Ok(())
}

fn reduce(pool: &PackagePool, res: &mut Vec<usize>, solver: &mut Solver, assumes: &mut Vec<Lit>) -> Result<()> {
    let old_badness = get_badness(pool, res);
    // Find all versions of a package
    res.iter().for_each(|pkg| {
        let pkgmeta = &pool.id_to_pkg(*pkg).unwrap();
        let ids = pool.pkg_name_to_ids(&pkgmeta.name).unwrap();
        let mut no_ids: Vec<Lit> = ids.into_iter().map(|(id, _)| !Lit::from_dimacs(id as isize)).collect();
        let mut new_assume = assumes.clone();
        new_assume.append(&mut no_ids);
        solver.assume(&new_assume);
        if solver.solve().unwrap() {
            let new_res = solve(solver).unwrap();
            let new_badness = get_badness(pool, &new_res);
            if new_badness <= old_badness {
                *assumes = new_assume;
            }
        }
    });

    solver.assume(assumes);
    *res = solve(solver).unwrap();
    // Reset solver
    solver.assume(&[]);
    Ok(())
}

/// Generate a list of Lit of all older packages
/// The idea is that with these assumptions, the SAT solver must choose more up-to-date
///   packages, or give Unsolvable
pub fn gen_update_assume(pool: &PackagePool, ids: &[usize]) -> Vec<Lit> {
    let mut res = Vec::new();
    for id in ids {
        if !is_best(pool, *id).unwrap() {
            // Find all newer versions of this package
            let name = &pool.id_to_pkg(*id).unwrap().name;
            let pkgids: Vec<usize> = pool
                .pkg_name_to_ids(name)
                .unwrap()
                .into_iter()
                .map(|pkg| pkg.0)
                .collect();

            let mut reached = false;
            for pkgid in pkgids {
                if pkgid == *id {
                    reached = true
                }
                if reached {
                    reached = true;
                    let lit = !Lit::from_dimacs(pkgid as isize);
                    res.push(lit);
                }
            }
        }
    }
    res
}

#[inline]
fn get_badness(pool: &PackagePool, pkgs: &[usize]) -> usize {
    pkgs.iter().map(|pkg| {
        if is_best(pool, *pkg).unwrap() {
            1
        } else {
            100
        }
    }).sum()
}

#[inline]
pub fn is_best(pool: &PackagePool, id: usize) -> Option<bool> {
    let name = &pool.id_to_pkg(id)?.name;
    let ids = pool.pkg_name_to_ids(name)?;
    if ids[0].0 != id {
        Some(false)
    } else {
        Some(true)
    }
}
