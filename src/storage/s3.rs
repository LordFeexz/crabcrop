use anyhow::{anyhow, Context, Result};
use aws_sdk_s3::Client as S3Client;
use bytes::Bytes;
use reqwest::Client as HttpClient;
use std::time::Duration;
use tracing::instrument;

#[derive(Clone)]
pub struct StorageClient {
    http: HttpClient,
    s3: Option<S3Client>,
}

impl StorageClient {
    pub fn new(s3_client: Option<S3Client>) -> Self {
        let http = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("crabcrop/1.0")
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            s3: s3_client,
        }
    }

    #[instrument(skip(self), fields(url = %url))]
    pub async fn fetch(&self, url: &str) -> Result<Bytes> {
        if url.starts_with("s3://") {
            self.fetch_s3(url).await
        } else {
            self.fetch_http(url).await
        }
    }

    async fn fetch_http(&self, url: &str) -> Result<Bytes> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET request failed: {url}"))?;

        let status = response.status();
        if status.as_u16() == 404 {
            return Err(anyhow!("image not found at: {url}"));
        }
        if !status.is_success() {
            return Err(anyhow!("upstream returned {status} for: {url}"));
        }

        if let Some(ct) = response.headers().get(reqwest::header::CONTENT_TYPE) {
            let ct_str = ct.to_str().unwrap_or("");
            if !ct_str.starts_with("image/") {
                return Err(anyhow!("upstream returned non-image content-type: {ct_str}"));
            }
        }

        let bytes = response
            .bytes()
            .await
            .context("reading response body failed")?;
        Ok(bytes)
    }

    async fn fetch_s3(&self, url: &str) -> Result<Bytes> {
        let s3 = self
            .s3
            .as_ref()
            .ok_or_else(|| anyhow!("S3 client not configured"))?;

        let without_scheme = url.strip_prefix("s3://").unwrap_or(url);
        let (bucket, key) = without_scheme
            .split_once('/')
            .ok_or_else(|| anyhow!("invalid S3 URL, expected s3://bucket/key: {url}"))?;

        let output = s3
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("S3 GetObject failed for s3://{bucket}/{key}"))?;

        let bytes = output
            .body
            .collect()
            .await
            .context("reading S3 object body failed")?
            .into_bytes();

        Ok(bytes)
    }
}
