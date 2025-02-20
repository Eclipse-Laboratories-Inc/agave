use futures::StreamExt;
use indicatif::ProgressBar;
use reqwest::StatusCode;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum FileDownloadError {
    #[error("HTTP request failed: {status_code}")]
    RequestFailed { status_code: StatusCode },

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

fn get_filename_from_url(url: &reqwest::Url) -> String {
    let path_segment = url.path_segments().unwrap().last().unwrap();
    path_segment.to_string()
}

/// Computes the output file path for the snapshot.
///
/// - If `output_path` has an extension, return `output_path`.
/// - If the response contains a Content-Disposition filename, return `<output_path>/<filename>`.
/// - Otherwise, return the last element of the URL.
fn compute_output_file_path(
    output_path: &Path,
    response: &reqwest::Response,
) -> Result<PathBuf, FileDownloadError> {
    if output_path.extension().is_some() {
        return Ok(output_path.to_path_buf());
    }

    // Need to get the URL from the response to handle redirects
    let url = response.url();
    let filename = get_filename_from_url(url);
    Ok(output_path.join(filename))
}

/// Asynchronously downloads a file from `url` and saves it to `out_path`.
/// Uses a temporary file name, then renames if successful.
pub async fn download_file(
    rpc_client: &reqwest::Client,
    url: &str,
    output_path: &Path,
    progress_bar: &ProgressBar,
) -> Result<PathBuf, FileDownloadError> {
    let response = rpc_client.get(url).send().await?;

    // Check if the server returned a success code (2xx)
    if !response.status().is_success() {
        return Err(FileDownloadError::RequestFailed {
            status_code: response.status(),
        });
    }

    // Get Content-Disposition to figure out the filename
    let output_file_path = compute_output_file_path(output_path, &response)?;
    if let Some(output_dir) = output_file_path.parent() {
        std::fs::create_dir_all(output_dir)?;
    }

    // (Optional) You can get the content length if the server sends it.
    let content_length = response.content_length().unwrap_or(0);

    progress_bar.set_length(content_length);

    // For safety, download to a temporary file, then rename.
    let tmp_path = format!("{}.download", output_file_path.to_string_lossy());

    // Create the file and wrap it in a buffered writer.
    // This way we write to disk in efficient chunks.
    let file = File::create(&tmp_path)?;
    // Opening a scope to drop the writer at the end before renaming the file.
    {
        let mut writer = BufWriter::new(file);

        // We'll read the response body in a streaming manner
        let mut stream = response.bytes_stream();

        // We'll use an 8KB buffer for each chunk. Adjust to your preference.
        // You could also just read each `Bytes` chunk from the stream in the loop below.
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?; // If there's an error in the stream, bubble it up
            writer.write_all(&chunk)?;
            progress_bar.inc(chunk.len() as u64);
        }

        // Ensure all data is written to disk
        writer.flush()?;
    }

    // Rename the temporary file to the final path
    std::fs::rename(&tmp_path, &output_file_path)?;

    progress_bar.finish_with_message(format!(
        "Download complete: {}",
        output_file_path.to_string_lossy()
    ));

    Ok(output_file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    #[rstest]
    fn test_get_filename_from_url() {
        let url = "https://eclipse-rpc.xyz/snapshot.tar.bz2";
        let parsed_url: reqwest::Url = reqwest::Url::parse(url).unwrap();
        let filename = get_filename_from_url(&parsed_url);
        assert_eq!(filename, "snapshot.tar.bz2");
    }

    #[ignore = "Downloads a large file"]
    #[rstest]
    #[tokio::test]
    async fn test_download_file() {
        let url = "https://mainnetbeta-rpc.eclipse.xyz/incremental-snapshot.tar.bz2";
        let rpc_client = reqwest::Client::new();

        let output_dir = tempfile::tempdir().unwrap();

        let progress_bar = ProgressBar::new(0);
        download_file(&rpc_client, url, output_dir.path(), &progress_bar)
            .await
            .unwrap();
    }
}
