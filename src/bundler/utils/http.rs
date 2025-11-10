//! HTTP utilities for downloading bundler tools.
//!
//! Provides functions for downloading files.

#[cfg(target_os = "linux")]
use crate::bundler::error::Result;

#[cfg(target_os = "linux")]
use crate::bundler::error::Error;

/// Downloads a file from a URL.
///
/// Returns the file contents as a byte vector.
///
/// Used by:
/// - Linux: AppImage bundler (downloads linuxdeploy tool)
#[cfg(target_os = "linux")]
pub async fn download(url: &str) -> Result<Vec<u8>> {
    log::info!("Downloading {}", url);

    let response = reqwest::get(url)
        .await
        .map_err(|e| Error::GenericError(format!("Download failed: {}", e)))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| Error::GenericError(format!("Failed to read response: {}", e)))?;

    Ok(bytes.to_vec())
}
