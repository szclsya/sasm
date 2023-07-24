use crate::pool::PkgPool;
use anyhow::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

/// Use trajan algorithm to find out installation order of packages
pub fn sort_pkgs(pool: &dyn PkgPool, pkgs: &mut Vec<usize>) -> Result<()> {
    let res = sort_pkgs_to_cycles(pool, pkgs)?;
    pkgs.clear();
    for mut pkgids in res {
        if pkgids.len() == 1 {
            pkgs.push(pkgids[0]);
        } else {
            // Sort via the number of dependencies
            pkgids.sort_by_key(|id| {
                let pkg = pool.get_pkg_by_id(*id).unwrap();
                pkg.depends.len()
            });
            pkgs.append(&mut pkgids);
        }
    }

    Ok(())
}

pub fn sort_pkgs_to_cycles(pool: &dyn PkgPool, pkgs: &[usize]) -> Result<Vec<Vec<usize>>> {
    let mut g = DiGraph::<usize, ()>::new();
    let mut indexs: HashMap<usize, NodeIndex> = HashMap::new();
    // Add package nodes
    for pkgid in pkgs.iter() {
        indexs.insert(*pkgid, g.add_node(*pkgid));
    }
    // Add dependency edges
    for pkgid in pkgs.iter() {
        let deps: Vec<usize> = pool.get_deps(*pkgid)?.into_iter().flatten().collect();
        for depid in deps {
            if pkgs.contains(&depid) {
                // Add a directed edge
                g.update_edge(indexs[pkgid], indexs[&depid], ());
            }
        }
    }
    // Find a path
    let solve_res = petgraph::algo::tarjan_scc(&g);

    let mut res = Vec::new();
    for pkg_indexs in solve_res {
        let cycle: Vec<usize> = pkg_indexs.into_iter().map(|index| g[index]).collect();
        res.push(cycle);
    }

    Ok(res)
}
