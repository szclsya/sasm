use anyhow::Result;
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

// Given a filename or path, find package names that provide such file
pub fn search_file(dbs: &[PathBuf], filename: &str) -> Result<Vec<String>> {
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
        let bufreader = BufReader::new(f);
        for line in bufreader.lines() {
            let line = line?;
            if let Some(captures) = regex.captures(&line) {
                res.push(captures.name("pkgname").unwrap().as_str().to_owned());
            }
        }
    }

    res.dedup();
    Ok(res)
}
