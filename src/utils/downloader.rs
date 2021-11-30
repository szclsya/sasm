use crate::{msg, types::Checksum};

use anyhow::{bail, format_err, Result};
use async_compression::tokio::write::{GzipDecoder, XzDecoder};
use futures_util::future::select_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::{
    fs::OpenOptions,
    io::{AsyncWrite, AsyncWriteExt},
};

#[derive(Clone)]
pub struct DownloadJob {
    pub url: String,
    pub description: Option<String>,
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub compression: Compression,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum Compression {
    Gzip((Option<Checksum>, Option<Checksum>)),
    Xz((Option<Checksum>, Option<Checksum>)),
    None(Option<Checksum>),
}

impl Compression {
    pub fn get_extracted_checksum(&self) -> Option<Checksum> {
        match self {
            Compression::Gzip((_, c)) => c,
            Compression::Xz((_, c)) => c,
            Compression::None(c) => c,
        }
        .clone()
    }

    pub fn get_download_checksum(&self) -> Option<Checksum> {
        match self {
            Compression::Gzip((c, _)) => c,
            Compression::Xz((c, _)) => c,
            Compression::None(c) => c,
        }
        .clone()
    }
}

pub struct Downloader {
    client: Client,
    max_concurrent: usize,
    max_retry: usize,
}

impl Downloader {
    pub fn new() -> Self {
        Downloader {
            client: Client::new(),
            max_concurrent: 5,
            max_retry: 3,
        }
    }

    /// Download all required stuff in an async manner and show a progress bar
    pub async fn fetch(
        &self,
        mut to_download: Vec<DownloadJob>,
        download_path: &Path,
    ) -> Result<HashMap<String, PathBuf>> {
        // Create download dir
        if !download_path.is_dir() {
            tokio::fs::create_dir_all(download_path).await?;
        }

        let mut position = (0, to_download.len(), to_download.len().to_string().len());
        let mut res = HashMap::new();
        // Handles for download processes
        let mut handles = Vec::with_capacity(self.max_concurrent);

        // Show download info
        msg!("", "Downloading {} files...", to_download.len());
        let multibar = MultiProgress::new();
        let bar_template = {
            let max_len = crate::WRITER.get_max_len();
            if max_len < 90 {
                " {wide_msg} {total_bytes:>10} {binary_bytes_per_sec:>12} {eta:>4} {percent:>3}%"
            } else {
                " {msg:<48} {total_bytes:>10} {binary_bytes_per_sec:>12} {eta:>4} [{wide_bar:.white/black}] {percent:>3}%"
            }
        };
        let barsty = ProgressStyle::default_bar()
            .template(bar_template)
            .progress_chars("=>-");
        while !to_download.is_empty() {
            while handles.len() < self.max_concurrent && !to_download.is_empty() {
                let job = to_download.pop().unwrap();
                let client = self.client.clone();
                let path = download_path.to_owned();
                let bar = multibar.insert(0, ProgressBar::new(job.size.unwrap_or(0)));
                bar.set_style(barsty.clone());
                position.0 += 1;
                let handle = tokio::spawn(async move {
                    try_download_file(client, path, job, 0, position, bar).await
                });
                handles.push(handle);
            }
            // Wait for any of them to stop
            let (download_res, _, remaining) = select_all(handles).await;
            handles = remaining;
            // Remove the handle from the list
            match download_res.unwrap() {
                Ok((name, path)) => {
                    res.insert(name, path);
                }
                Err(err) => {
                    // Handling download errors
                    // If have remaining reties, do it
                    if err.retry < self.max_retry {
                        let c = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(c, path, err.job, err.retry + 1, err.pos, err.bar)
                                .await
                        });
                        handles.push(handle);
                    } else {
                        return Err(err.error);
                    }
                }
            }
        }
        // Wait for the remaining to finish
        while !handles.is_empty() {
            let (download_res, _, remaining) = select_all(handles).await;
            handles = remaining;
            match download_res.unwrap() {
                Ok((url, path)) => {
                    res.insert(url, path);
                }
                Err(err) => {
                    // Handling download errors
                    // If have remaining reties, do it
                    if err.retry < self.max_retry {
                        let c = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(c, path, err.job, err.retry + 1, err.pos, err.bar)
                                .await
                        });
                        handles.push(handle);
                    } else {
                        return Err(err.error);
                    }
                }
            }
        }
        Ok(res)
    }
}

struct DownloadError {
    error: anyhow::Error,
    job: DownloadJob,
    retry: usize,
    pos: (usize, usize, usize),
    bar: ProgressBar,
}

async fn try_download_file(
    client: Client,
    path: PathBuf,
    job: DownloadJob,
    retry: usize,
    pos: (usize, usize, usize),
    bar: ProgressBar,
) -> Result<(String, PathBuf), DownloadError> {
    match download_file(&client, &path, job.clone(), pos, bar.clone()).await {
        Ok(res) => Ok(res),
        Err(error) => Err({
            bar.reset();
            DownloadError {
                error,
                job,
                retry: retry + 1,
                pos,
                bar,
            }
        }),
    }
}

async fn download_file(
    client: &Client,
    path: &Path,
    job: DownloadJob,
    pos: (usize, usize, usize),
    bar: ProgressBar,
) -> Result<(String, PathBuf)> {
    let mut resp = client.get(&job.url).send().await?;
    resp.error_for_status_ref()?;
    let filename = match job.filename {
        Some(n) => n,
        None => resp
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .ok_or_else(|| format_err!("{} doesn't contain filename", &job.url))?
            .to_string(),
    };
    let len = match job.size {
        Some(len) => len,
        None => resp
            .content_length()
            .ok_or_else(|| format_err!("Cannot determine content length"))?,
    };
    let msg = job.description.as_ref().unwrap_or(&filename);

    let file_path = path.join(&filename);
    let mut f = {
        if file_path.is_file() {
            if let Some(checksum) = job.compression.get_extracted_checksum() {
                let p = file_path.clone();
                let res = tokio::task::spawn_blocking(move || checksum.cmp_file(&p)).await?;
                if res.is_ok() && res.unwrap() {
                    bar.finish_and_clear();
                    bar.println(format!(
                        "{}{} (not modified)",
                        crate::cli::gen_prefix(&console::style("SKIP").dim().to_string()),
                        &msg
                    ));
                    return Ok((job.url, file_path));
                }
            }
            // If checksum DNE/mismatch, purge current content
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(true)
                .open(&file_path)
                .await?;
            f.set_len(0).await?;
            f
        } else {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&file_path)
                .await?
        }
    };

    // Prepare progress bar
    let mut progress_text = format!("({:0width$}/{}) {}", pos.0, pos.1, msg, width = pos.2);
    if console::measure_text_width(&progress_text) > 48 {
        progress_text = console::truncate_str(&progress_text, 45, "...").to_string();
    }
    bar.set_message(progress_text);
    bar.set_length(len);
    bar.set_position(0);
    bar.reset();

    // Download!
    {
        let mut validator = job
            .compression
            .get_download_checksum()
            .as_ref()
            .map(|c| c.get_validator());
        let mut writer: Box<dyn AsyncWrite + Unpin + Send> = match job.compression {
            Compression::Gzip(_) => Box::new(GzipDecoder::new(&mut f)),
            Compression::Xz(_) => Box::new(XzDecoder::new(&mut f)),
            Compression::None(_) => Box::new(&mut f),
        };
        while let Some(chunk) = resp.chunk().await? {
            writer.write_all(&chunk).await?;
            bar.inc(chunk.len() as u64);
            if let Some(ref mut validator) = validator {
                validator.update(&chunk);
            }
        }
        writer.shutdown().await?;

        if let Some(len) = job.size {
            if bar.length() != len {
                bail!(
                    "Bad file size when downloading {}. Mirror may be syncing. Try again later.",
                    job.url
                );
            }
        }

        if let Some(validator) = validator {
            // finish() returns false if validate failed
            if !validator.finish() {
                bail!("Checksum mismatch for file {}", filename);
            }
        }
    }

    bar.finish_and_clear();
    bar.println(format!(
        "{}{}",
        crate::cli::gen_prefix(&console::style("DONE").dim().to_string()),
        &msg
    ));
    Ok((job.url, file_path))
}
