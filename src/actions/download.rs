use crate::{
    db::LocalDb,
    info, pool,
    types::{Checksum, PkgSource},
    utils::downloader::{Compression, DownloadJob, Downloader},
};

use anyhow::{bail, Context, Result};
use console::style;

pub async fn download(
    pkgname: &str,
    local_db: &LocalDb,
    downloader: &Downloader,
    latest: bool,
) -> Result<()> {
    let dbs = local_db
        .get_all_package_db()
        .context("Invalid local package database")?;
    let pool = pool::source::create_pool(&dbs, &[])?;

    // Get all versions
    // Choices: Vec<(DisplayString, URL)>
    let mut choices: Vec<(String, String, u64, Checksum)> = Vec::new();
    if let Some(ids) = pool.get_pkgs_by_name(pkgname) {
        let mut first = true;
        for id in ids {
            let meta = pool.get_pkg_by_id(id).unwrap();
            let (url, size, checksum) = match &meta.source {
                PkgSource::Http((url, size, checksum)) => (url, size, checksum),
                // This should never happen
                PkgSource::Local(_) => panic!("Local source from http repo"),
            };
            // Form version str for display
            let mut version_str = meta.version.to_string();
            let mut info_segments = Vec::new();
            if first {
                info_segments.push(style("latest").green().to_string());
            }
            if !info_segments.is_empty() {
                version_str.push_str(&format!(" ({})", info_segments.join(", ")));
            }
            choices.push((version_str, url.to_owned(), *size, checksum.to_owned()));
            // Not the first anymore
            first = false;
        }
    } else {
        bail!("Package {} not found", style(pkgname).bold());
    }

    // Display them
    let choices_str: Vec<&str> = choices.iter().map(|ver| ver.0.as_str()).collect();
    let i = if latest {
        0
    } else {
        info!("Please choose a version for {}:", style(pkgname).bold());
        dialoguer::Select::with_theme(&crate::cli::OmaTheme::default())
            .items(&choices_str)
            .default(0)
            .interact()?
    };

    let (_, url, size, checksum) = &choices[i];
    let job = DownloadJob {
        url: url.to_owned(),
        description: None,
        filename: None,
        size: Some(*size),
        compression: Compression::None(Some(checksum.clone())),
    };

    // Download package to current directory
    let current_dir = std::env::current_dir().context("Failed to get current directory.")?;
    downloader
        .fetch([job].to_vec(), &current_dir, true)
        .await
        .context("Failed to fetch request package from repository.")?;

    Ok(())
}
