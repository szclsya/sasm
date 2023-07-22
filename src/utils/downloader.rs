use crate::{msg, types::Checksum};

use anyhow::{bail, format_err, Result};
use async_compression::tokio::write::{GzipDecoder, XzDecoder};
use console::style;
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
            Compression::Gzip((_, c)) | Compression::Xz((_, c)) | Compression::None(c) => c,
        }
        .clone()
    }

    pub fn get_download_checksum(&self) -> Option<Checksum> {
        match self {
            Compression::Gzip((c, _)) | Compression::Xz((c, _)) | Compression::None(c) => c,
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
        global_progess: bool,
    ) -> Result<HashMap<String, PathBuf>> {
        // Create download dir
        if !download_path.is_dir() {
            tokio::fs::create_dir_all(download_path).await?;
        }

        // Calculate total size
        let total_size: u64 = to_download.iter().map(|job| job.size.unwrap_or(0)).sum();

        let mut res = HashMap::new();
        // Handles for download processes
        let mut handles = Vec::with_capacity(self.max_concurrent);

        // Show download info
        msg!("Downloading {} files...", to_download.len());
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
            .template(bar_template)?
            .progress_chars("=>-");
        // Create a global bar if some files specified size
        let total = to_download.len();
        let total_str_len = total.to_string().len();
        let mut finished = 0;
        let global_bar = if total_size > 0 && global_progess {
            let bar = multibar.insert(0, ProgressBar::new(total_size));
            bar.set_style(barsty.clone());
            Some(bar)
        } else {
            None
        };

        // Down them all!
        while !to_download.is_empty() {
            while handles.len() < self.max_concurrent && !to_download.is_empty() {
                let job = to_download.pop().unwrap();
                let client = self.client.clone();
                let path = download_path.to_owned();
                let bar = multibar.insert(0, ProgressBar::new(job.size.unwrap_or(0)));
                let global_bar = global_bar.clone();
                bar.set_style(barsty.clone());
                let handle = tokio::spawn(async move {
                    try_download_file(client, path, job, 0, bar, global_bar).await
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
                    finished += 1;
                    update_global_bar(&global_bar, total, finished, total_str_len);
                }
                Err(e) => {
                    // Handling download errors
                    // If have remaining reties, do it
                    if e.retry < self.max_retry {
                        let c = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(c, path, e.job, e.retry + 1, e.bar, e.global_bar)
                                .await
                        });
                        handles.push(handle);
                    } else {
                        return Err(e.error);
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
                    finished += 1;
                    update_global_bar(&global_bar, total, finished, total_str_len);
                }
                Err(e) => {
                    // Handling download errors
                    // If have remaining reties, do it
                    if e.retry < self.max_retry {
                        let c = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(c, path, e.job, e.retry + 1, e.bar, e.global_bar)
                                .await
                        });
                        handles.push(handle);
                    } else {
                        return Err(e.error);
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
    bar: ProgressBar,
    global_bar: Option<ProgressBar>,
}

async fn try_download_file(
    client: Client,
    path: PathBuf,
    job: DownloadJob,
    retry: usize,
    bar: ProgressBar,
    global_bar: Option<ProgressBar>,
) -> Result<(String, PathBuf), DownloadError> {
    match download_file(&client, &path, job.clone(), bar.clone(), global_bar.clone()).await {
        Ok(res) => Ok(res),
        Err(error) => Err({
            bar.reset();
            DownloadError {
                error,
                job,
                retry: retry + 1,
                bar,
                global_bar,
            }
        }),
    }
}

async fn download_file(
    client: &Client,
    path: &Path,
    job: DownloadJob,
    bar: ProgressBar,
    global_bar: Option<ProgressBar>,
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
            .ok_or_else(|| format_err!("{} doesn't contain filename.", &job.url))?
            .to_string(),
    };
    let len = match job.size {
        Some(len) => len,
        None => resp
            .content_length()
            .ok_or_else(|| format_err!("Cannot determine content length."))?,
    };
    let msg = job.description.as_ref().unwrap_or(&filename);

    let file_path = path.join(&filename);
    let mut f = {
        if file_path.is_file() {
            if let Some(checksum) = job.compression.get_extracted_checksum() {
                let p = file_path.clone();
                let res = tokio::task::spawn_blocking(move || checksum.cmp_file(&p)).await?;
                if res.is_ok() && res.unwrap() {
                    // Checksum matched.
                    bar.finish_and_clear();
                    // Reduce global bar length, since we don't need to download this file
                    if let Some(ref global_bar) = global_bar {
                        global_bar.set_length(global_bar.length().unwrap() - len);
                    }

                    if crate::verbose() || global_bar.is_some() {
                        bar.println(format!(
                            "{}{} (not modified)",
                            crate::cli::gen_prefix(&console::style("SKIP").dim().to_string()),
                            &msg
                        ));
                    }
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
    let mut progress_text = msg.to_owned();
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
            let len = chunk.len().try_into().unwrap();
            bar.inc(len);
            // Increase global bar, if applicable
            if let Some(ref global_bar) = global_bar {
                global_bar.inc(len);
            }
            if let Some(ref mut validator) = validator {
                validator.update(&chunk);
            }
        }
        writer.shutdown().await?;

        if let Some(len) = job.size {
            if bar.length().unwrap() != len {
                bail!(
                    "Bad file size when downloading {}: mirrors may be synchronizing, please try again later.",
                    job.url
                );
            }
        }

        if let Some(validator) = validator {
            // finish() returns false if validate failed
            if !validator.finish() {
                bail!("Checksum mismatched for file {}.", filename);
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

#[inline]
fn update_global_bar(
    bar: &Option<ProgressBar>,
    total: usize,
    finished: usize,
    total_text_len: usize,
) {
    if let Some(bar) = bar {
        bar.set_message(
            style(gen_global_bar_message(total, finished, total_text_len))
                .bright()
                .bold()
                .to_string(),
        );
    }
}

#[inline]
fn gen_global_bar_message(total: usize, finished: usize, total_text_len: usize) -> String {
    let finished_str = finished.to_string();
    format!(
        "Total Progress: [{: >width$}/{}]",
        finished_str,
        total,
        width = total_text_len
    )
}
