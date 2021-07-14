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
    for mut pkg_indexs in solve_res {
        if pkg_indexs.len() == 1 {
            pkgs.push(g[pkg_indexs[0]]);
        } else {
            pkg_indexs.sort_by_key(|index| pool.get_deps(g[*index]).unwrap().len());
            pkgs.extend(pkg_indexs.into_iter().map(|index| g[index]));
        }
    }

    Ok(())
}
