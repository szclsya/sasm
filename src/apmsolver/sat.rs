use super::pool::PackagePool;
use super::types::*;
use super::version::PackageVersion;
use std::collections::HashMap;

fn generate(req: &Request, pool: &PackagePool) {
    // The id-pkg table
    let mut idpkg: HashMap<usize, (String, PackageVersion)> = HashMap::new();
    // Enroll requirements
    for r in &req.install {
        // Pick the latest version for each requested packages
        let choices = pool.get_ids(&r.0);
    }
}
