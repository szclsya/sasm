use super::PackagePool;
use anyhow::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

/// Use trajan algorithm to find out installation order of packages
pub fn sort_pkgs(pool: &PackagePool, pkgs: &mut Vec<usize>) -> Result<()> {
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

    // Reorder pkgs
    pkgs.clear();
    for pkg_indexs in solve_res {
        for pkgnode in pkg_indexs {
            pkgs.push(g[pkgnode]);
        }
    }

    Ok(())
}
