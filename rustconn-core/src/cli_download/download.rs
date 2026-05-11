use super::{
    CliDownloadError, CliDownloadResult, DownloadCancellation, DownloadProgress, ProgressCallback,
};

/// Download file with progress reporting
pub(super) async fn download_with_progress(
    url: &str,
    progress_callback: &ProgressCallback,
    cancel_token: &DownloadCancellation,
) -> CliDownloadResult<Vec<u8>> {
    use futures::StreamExt;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| CliDownloadError::DownloadFailed(e.to_string()))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| CliDownloadError::DownloadFailed(e.to_string()))?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        return Err(CliDownloadError::DownloadFailed(format!(
            "HTTP {} - {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown error")
        )));
    }

    let total_size = response.content_length();
    let mut downloaded: u64 = 0;
    let mut data = Vec::with_capacity(total_size.unwrap_or(1_000_000) as usize);

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Err(CliDownloadError::Cancelled);
        }

        let chunk = chunk_result.map_err(|e| CliDownloadError::DownloadFailed(e.to_string()))?;
        downloaded += chunk.len() as u64;
        data.extend_from_slice(&chunk);

        if let Some(cb) = progress_callback {
            cb(DownloadProgress {
                downloaded,
                total: total_size,
                status: format!("Downloading... {:.1} MB", downloaded as f64 / 1_000_000.0),
            });
        }
    }

    Ok(data)
}

/// Verify SHA256 checksum of downloaded data
pub(super) fn verify_checksum(data: &[u8], expected: &str) -> CliDownloadResult<()> {
    use ring::digest::{Context, SHA256};

    let mut context = Context::new(&SHA256);
    context.update(data);
    let digest = context.finish();
    let actual = hex::encode(digest.as_ref());

    if actual != expected {
        return Err(CliDownloadError::ChecksumMismatch {
            expected: expected.to_string(),
            actual,
        });
    }

    Ok(())
}
