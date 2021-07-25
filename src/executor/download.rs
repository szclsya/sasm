use crate::msg;
use anyhow::{format_err, Result};
use futures::future::select_all;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::Client;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::{fs::File, io::AsyncWriteExt};

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
    /// to_download: Vec<(URL, Option<DesiredFilename>, Option<size>)
    pub async fn fetch(
        &self,
        mut to_download: Vec<(String, Option<String>, Option<u64>)>,
        download_path: &Path,
    ) -> Result<HashMap<String, PathBuf>> {
        // Create download dir
        if !download_path.is_dir() {
            tokio::fs::create_dir_all(download_path).await?;
        }

        let mut res = HashMap::new();
        // Handles for download processes
        let mut handles = Vec::with_capacity(self.max_concurrent);

        // Show download info
        msg!("", "Downloading {} files...", to_download.len());
        let multibar = MultiProgress::new();
        let barsty = ProgressStyle::default_bar()
            .template(" {msg:<33} {total_bytes:>10} {binary_bytes_per_sec:>10} {eta:>4} [{wide_bar:.white/black}] {percent:<3}%")
            .progress_chars("=>-");
        while !to_download.is_empty() {
            while handles.len() < self.max_concurrent && !to_download.is_empty() {
                let (url, filename, len) = to_download.pop().unwrap();
                let client = self.client.clone();
                let path = download_path.to_owned();
                let bar = multibar.insert(0, ProgressBar::new(len.unwrap_or(0)));
                bar.set_style(barsty.clone());
                let handle = tokio::spawn(async move {
                    try_download_file(client, path, url, filename, len, 0, bar).await
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
                        let client = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(
                                client,
                                path,
                                err.url,
                                err.filename,
                                err.len,
                                err.retry + 1,
                                err.bar,
                            )
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
                        let client = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(
                                client,
                                path,
                                err.url,
                                err.filename,
                                err.len,
                                err.retry + 1,
                                err.bar,
                            )
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
    url: String,
    filename: Option<String>,
    len: Option<u64>,
    retry: usize,
    bar: ProgressBar,
}

async fn try_download_file(
    client: Client,
    path: PathBuf,
    url: String,
    filename: Option<String>,
    len: Option<u64>,
    retry: usize,
    bar: ProgressBar,
) -> Result<(String, PathBuf), DownloadError> {
    match download_file(
        &client,
        &path,
        url.clone(),
        filename.clone(),
        len,
        bar.clone(),
    )
    .await
    {
        Ok(res) => Ok(res),
        Err(error) => Err({
            bar.reset();
            DownloadError {
                error,
                url,
                filename,
                len,
                retry: retry + 1,
                bar,
            }
        }),
    }
}

async fn download_file(
    client: &Client,
    path: &Path,
    url: String,
    filename: Option<String>,
    len: Option<u64>,
    bar: ProgressBar,
) -> Result<(String, PathBuf)> {
    let mut resp = client.get(&url).send().await?;
    resp.error_for_status_ref()?;
    let filename = match filename {
        Some(n) => n,
        None => resp
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .ok_or_else(|| format_err!("{} doesn't contain filename", &url))?
            .to_string(),
    };
    let file_path = path.join(&filename);
    let mut f = File::create(&file_path).await?;

    let len = match len {
        Some(len) => len,
        None => resp
            .content_length()
            .ok_or_else(|| format_err!("Cannot determine content length"))?,
    };

    // Begin the download!
    let mut msg = filename.clone();
    if filename.len() > 33 {
        msg.truncate(30);
        msg.push_str("...");
    }
    bar.set_message(msg);
    bar.set_length(len);
    bar.set_position(0);
    bar.reset();
    while let Some(chunk) = resp.chunk().await? {
        f.write_all(&chunk).await?;
        bar.inc(chunk.len() as u64);
    }
    bar.finish_and_clear();
    bar.println(format!("{:>9} {:<30}", "FINISHED", &filename));

    Ok((url.to_string(), file_path))
}
