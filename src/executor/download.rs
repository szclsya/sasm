use crate::success;
use anyhow::{format_err, Result};
use futures::future::{select, select_all};
use indicatif::MultiProgress;
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

    /// Download all required stuff in an async manner and show a progress bar (TODO)
    /// to_download: Vec<(Name, URL, Option<size>)
    pub async fn fetch(
        &self,
        mut to_download: Vec<(String, String, Option<u64>)>,
        download_path: &Path,
    ) -> Result<HashMap<String, PathBuf>> {
        // Create download dir
        if !download_path.is_dir() {
            tokio::fs::create_dir_all(download_path).await?;
        }

        let mut res = HashMap::new();
        // Handles for download processes
        let mut handles = Vec::with_capacity(self.max_concurrent);

        while !to_download.is_empty() {
            while handles.len() < self.max_concurrent && !to_download.is_empty() {
                let (name, url, len) = to_download.pop().unwrap();
                let client = self.client.clone();
                let path = download_path.to_owned();
                let handle =
                    tokio::spawn(async move { try_download_file(client, path, name, url, len, 0).await });
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
                    if err.retries < self.max_retry {
                        let client = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(
                                client,
                                path,
                                err.name,
                                err.url,
                                err.len,
                                err.retries + 1,
                            ).await
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
                    if err.retries < self.max_retry {
                        let client = self.client.clone();
                        let path = download_path.to_owned();
                        let handle = tokio::spawn(async move {
                            try_download_file(
                                client,
                                path,
                                err.name,
                                err.url,
                                err.len,
                                err.retries + 1,
                            ).await
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
    name: String,
    url: String,
    len: Option<u64>,
    retries: usize,
}

async fn try_download_file(
    client: Client,
    path: PathBuf,
    name: String,
    url: String,
    len: Option<u64>,
    retries: usize,
) -> Result<(String, PathBuf), DownloadError> {
    match download_file(&client, &name, &path, url.clone(), len).await {
        Ok(res) => Ok(res),
        Err(error) => Err(DownloadError {
            error,
            name,
            url,
            len,
            retries: retries + 1,
        }),
    }
}

async fn download_file(
    client: &Client,
    name: &str,
    path: &Path,
    url: String,
    len: Option<u64>,
) -> Result<(String, PathBuf)> {
    let mut resp = client.get(&url).send().await?;
    resp.error_for_status_ref()?;
    let filename = resp
        .url()
        .path_segments()
        .and_then(|segments| segments.last())
        .and_then(|name| if name.is_empty() { None } else { Some(name) })
        .ok_or_else(|| format_err!("{} doesn't contain filename", &url))?
        .to_string();
    let file_path = path.join(&filename);
    let mut f = File::create(&file_path).await?;

    let len = match len {
        Some(len) => len,
        None => resp
            .content_length()
            .ok_or_else(|| format_err!("Cannot determine content length"))?,
    };

    let mut current_len = 0;
    while let Some(chunk) = resp.chunk().await? {
        f.write_all(&chunk).await?;
        current_len += chunk.len();
    }

    success!("{}", &filename);
    Ok((name.to_string(), file_path))
}
