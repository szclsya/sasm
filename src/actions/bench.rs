use crate::{
    db::LocalDb,
    info,
    types::{
        config::{Config, Opts},
        Checksum, ChecksumValidator,
    },
    utils::{downloader::Downloader, pager::Pager},
    warn,
};

use anyhow::{bail, Result};
use console::style;
use dialoguer::Confirm;
use indicatif::HumanBytes;
use reqwest::Client;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};
use tabled::{Alignment, Full, Head, Header, Modify, Style, Table, Tabled};

pub async fn bench(
    opts: &Opts,
    config: &Config,
    db: LocalDb,
    downloader: &Downloader,
) -> Result<()> {
    // First, update local db
    db.update(downloader).await?;

    let client = Client::new();
    let mut config = config.clone();
    let mut results = Vec::new();
    for (name, repo) in &mut config.repo {
        info!("Running benchmarking for repository {}", style(name).bold());
        let mut res = Vec::new();
        // Fetch Contents-all.gz for specified repo
        let contents_filename = format!(
            "Contents_{}_{}_{}.gz",
            repo.distribution, repo.components[0], config.arch
        );
        // Get ChecksumValidator for this file
        let local = db.get_contents_db(name)?;
        let local_paths: Vec<PathBuf> = local
            .into_iter()
            .filter(|(_, path)| path.ends_with(&contents_filename))
            .map(|(_, path)| path)
            .collect();
        if local_paths.is_empty() {
            bail!(
                "Internal Error: Local repository don't have benchmark file {}",
                contents_filename
            );
        }
        let local_path = &local_paths[0];
        let size = fs::metadata(&local_path)?.len();
        let local_hash = Checksum::from_file_sha256(local_path)?;
        let validator = local_hash.get_validator();

        for url in &repo.url {
            let contents_url = format!(
                "{}/dists/{}/{}/Contents-{}.gz",
                url, repo.distribution, repo.components[0], config.arch
            );
            // Start counting
            let start = Instant::now();
            match try_download(&contents_url, &client, validator.clone()).await {
                Ok(_) => {
                    let time = start.elapsed();
                    res.push((url.clone(), Some(time)));
                }
                Err(e) => {
                    warn!("Mirror {} failed to complete benchmark: {}", url, e);
                    res.push((url.clone(), None));
                }
            }
        }
        // Sort result based on time
        res.sort_by_key(|(_, time)| time.unwrap_or(Duration::MAX));
        // Generate new urls
        let new_urls = res.iter().map(|(url, _)| url.clone()).collect();
        repo.url = new_urls;
        // Push result of this repo to results
        results.push((name.as_str(), size, res));
    }

    // Show results
    show_bench_results(results.as_slice())?;

    // Ask if to write back results
    if Confirm::new()
        .with_prompt(format!(
            "{}{}",
            crate::cli::gen_prefix(""),
            "Apply optimal mirrors?"
        ))
        .interact()?
    {
        let new_config = toml::to_string(&config)?;
        let config_path = opts
            .root
            .join(&opts.config_root)
            .canonicalize()
            .unwrap()
            .join("config.toml");
        std::fs::write(config_path, new_config)?;
    }

    Ok(())
}

#[inline]
async fn try_download(url: &str, client: &Client, mut validator: ChecksumValidator) -> Result<()> {
    let mut resp = client.get(url).send().await?;
    while let Some(chunk) = resp.chunk().await? {
        validator.update(&chunk);
    }

    if !validator.finish() {
        bail!("Checksum mismatched.");
    }

    Ok(())
}

#[derive(Tabled)]
struct BenchResultRow {
    #[header("Best")]
    best: String,
    #[header("Mirror")]
    url: String,
    #[header("Speed")]
    speed: String,
}

#[inline]
fn show_bench_results(results: &[(&str, u64, Vec<(String, Option<Duration>)>)]) -> Result<()> {
    let mut pager = Pager::new()?;
    let writer = pager.get_writer()?;

    for (name, size, repo_results) in results {
        let mut rows = Vec::new();
        for (i, result) in repo_results.iter().enumerate() {
            let speed = if let Some(duration) = result.1 {
                let ms = duration.as_millis();
                // *1024 because ms to s
                let bytes_per_sec: u128 = *size as u128 / ms * 1024;
                format!("{}/s", HumanBytes(bytes_per_sec as u64))
            } else {
                style("FAILED").red().bold().to_string()
            };
            let best = if i == 0 {
                style("*").green().bold().to_string()
            } else {
                String::new()
            };
            let row = BenchResultRow {
                best,
                url: result.0.clone(),
                speed,
            };
            rows.push(row);
        }
        let table = Table::new(&rows)
            .with(Header(format!(
                "Benchmark Result for {}",
                style(name).bold()
            )))
            .with(Modify::new(Full).with(Alignment::left()))
            .with(Modify::new(Head).with(Alignment::center_horizontal()))
            .with(Modify::new(Full).with(|s: &str| format!(" {} ", s)))
            .with(Style::pseudo_clean());
        writeln!(writer, "{}\n", table)?;
    }

    pager.wait_for_exit()?;

    Ok(())
}
