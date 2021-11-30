use anyhow::Result;
use flate2::read::GzDecoder;
use rayon::prelude::*;
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

// Given a filename or path, find package names that provide such file
pub fn provide_file(dbs: &[PathBuf], filename: &str) -> Result<Vec<String>> {
    // Construct regex based on deb Contents file format
    let regex = if filename.starts_with('/') {
        // Absolute path, strip "/" to match Contents file format
        let path = filename.strip_prefix('/').unwrap();
        Regex::new(&format!(r"^{} +(?P<pkgname>[-a-zA-Z0-9.+/]+)$", path))?
    } else {
        // Relative path, allow segments before filename
        Regex::new(&format!(r"^.*{} +(?P<pkgname>[-a-zA-Z0-9.+/]+)$", filename))?
    };

    let mut res = Vec::new();
    for db in dbs {
        let f = File::open(db)?;
        let f = GzDecoder::new(f);
        let bufreader = BufReader::new(f);
        let mut pkgnames: Vec<String> = bufreader
            .lines()
            .par_bridge()
            .filter_map(|line| match line {
                Ok(l) => {
                    if regex.is_match(&l) {
                        let captures = regex.captures(&l).unwrap();
                        Some(captures.name("pkgname").unwrap().as_str().to_owned())
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
            .collect();
        res.append(&mut pkgnames);
    }

    res.dedup();
    Ok(res)
}
