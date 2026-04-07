use crate::error::{Error, Result};
use crate::wu_protocol::WUProtocol;
use futures_util::StreamExt;
use std::path::Path;
use std::{fs::File, io::Write};

pub struct VersionDownloader {
    client: reqwest::Client,
    protocol: WUProtocol,
}

impl VersionDownloader {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent("mclauncher/0.1")
            .build()?;
        Ok(Self {
            client,
            protocol: WUProtocol::default(),
        })
    }

    pub fn set_msa_user_token(&mut self, token: String) {
        self.protocol.set_msa_user_token(token);
    }

    async fn post_xml_async(&self, url: &str, data: &str) -> Result<String> {
        let resp = self
            .client
            .post(url)
            .header("Content-Type", "application/soap+xml")
            .body(data.to_string())
            .send()
            .await?;
        let resp = resp.error_for_status()?;
        Ok(resp.text().await?)
    }

    async fn get_download_url(&self, update_identity: &str, revision_number: &str) -> Result<Option<String>> {
        let request = self
            .protocol
            .build_download_request(update_identity, revision_number)?;
        let response = self
            .post_xml_async(self.protocol.download_url(), &request)
            .await?;
        let urls = WUProtocol::extract_download_response_urls(&response)?;
        for url in urls {
            if url.starts_with("http://tlu.dl.delivery.mp.microsoft.com/") {
                return Ok(Some(url));
            }
        }
        Ok(None)
    }

    pub async fn download_appx<F>(
        &self,
        update_identity: &str,
        revision_number: &str,
        destination: &Path,
        progress: F,
    ) -> Result<()>
    where
        F: FnMut(u64, Option<u64>) + Send,
    {
        let link = self
            .get_download_url(update_identity, revision_number)
            .await?
            .ok_or(Error::DownloadUrlNotFound)?;
        self.download_file(&link, destination, progress).await
    }

    pub async fn download_msixvc<F>(
        &self,
        download_urls: &[String],
        destination: &Path,
        progress: F,
    ) -> Result<()>
    where
        F: FnMut(u64, Option<u64>) + Send,
    {
        let url = download_urls.first().ok_or(Error::NoDownloadUrls)?;
        self.download_file(url, destination, progress).await
    }

    async fn download_file<F>(&self, url: &str, destination: &Path, mut progress: F) -> Result<()>
    where
        F: FnMut(u64, Option<u64>) + Send,
    {
        let resp = self.client.get(url).send().await?;
        let resp = resp.error_for_status()?;
        let total = resp.content_length();
        let mut stream = resp.bytes_stream();
        let mut file = File::create(destination)?;
        let mut downloaded = 0u64;
        progress(downloaded, total);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            progress(downloaded, total);
        }
        Ok(())
    }
}
