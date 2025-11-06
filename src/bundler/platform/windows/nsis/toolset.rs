//! NSIS toolset acquisition and management.
//!
//! Handles downloading and locating the NSIS toolset:
//! - Windows: Downloads NSIS from GitHub releases and caches locally
//! - Linux/macOS: Locates system-installed makensis binary

use crate::bundler::error::{Context, Error, ErrorExt, Result};
use std::path::PathBuf;

#[cfg(windows)]
use crate::bundler::utils::http;

// NSIS download constants (Windows only)
#[cfg(windows)]
const NSIS_URL: &str =
    "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3/nsis-3.zip";
#[cfg(windows)]
const NSIS_SHA1: &str = "057e83c7d82462ec394af76c87d06733605543d4";

/// Get or download NSIS toolset.
///
/// On Windows: Downloads NSIS from GitHub if not cached.
/// On Linux/macOS: Locates system-installed makensis.
///
/// Returns the path to the NSIS directory containing makensis executable.
pub async fn get_nsis_toolset() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        get_nsis_windows().await
    }

    #[cfg(not(windows))]
    {
        get_nsis_unix()
    }
}

/// Download and cache NSIS on Windows.
#[cfg(windows)]
async fn get_nsis_windows() -> Result<PathBuf> {
    // Determine cache directory
    let tools_dir = dirs::cache_dir()
        .ok_or_else(|| Error::GenericError("Could not find cache directory".into()))?
        .join("kodegen")
        .join("nsis");

    let nsis_path = tools_dir.join("NSIS");

    // Check if already downloaded
    if nsis_path.exists() && nsis_path.join("makensis.exe").exists() {
        log::debug!("NSIS found at {}", nsis_path.display());
        return Ok(nsis_path);
    }

    // Download NSIS
    log::info!("Downloading NSIS toolset...");

    let data = http::download_and_verify(NSIS_URL, NSIS_SHA1, http::HashAlgorithm::Sha1)
        .await
        .context("failed to download NSIS")?;

    // Extract
    log::info!("Extracting NSIS...");
    tokio::fs::create_dir_all(&tools_dir)
        .await
        .fs_context("creating NSIS tools directory", &tools_dir)?;
    http::extract_zip(&data, &tools_dir)
        .await
        .context("failed to extract NSIS")?;

    // Rename extracted folder (handle version-specific naming)
    let extracted = tools_dir.join("nsis-3.08");
    if extracted.exists() {
        tokio::fs::rename(&extracted, &nsis_path)
            .await
            .fs_context("renaming NSIS directory", &extracted)?;
    } else {
        // Try to find any nsis-* folder
        let mut entries = tokio::fs::read_dir(&tools_dir)
            .await
            .fs_context("reading tools directory", &tools_dir)?;

        let mut found = false;
        while let Some(entry) = entries
            .next_entry()
            .await
            .fs_context("reading directory entry", &tools_dir)?
        {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("nsis") {
                tokio::fs::rename(entry.path(), &nsis_path)
                    .await
                    .fs_context("renaming NSIS directory", &entry.path())?;
                found = true;
                break;
            }
        }

        if !found {
            return Err(Error::GenericError(
                "NSIS extraction failed: no nsis folder found in archive".into(),
            ));
        }
    }

    // Verify makensis.exe exists
    if !nsis_path.join("makensis.exe").exists() {
        return Err(Error::GenericError(format!(
            "NSIS installation incomplete: makensis.exe not found at {}",
            nsis_path.display()
        )));
    }

    Ok(nsis_path)
}

/// Locate system-installed makensis on Unix systems.
#[cfg(not(windows))]
fn get_nsis_unix() -> Result<PathBuf> {
    // On Linux/macOS, find system-installed makensis
    match which::which("makensis") {
        Ok(path) => {
            let bin_dir = path.parent().ok_or_else(|| {
                Error::GenericError("makensis path has no parent directory".into())
            })?;
            Ok(bin_dir.to_path_buf())
        }
        Err(_) => Err(Error::GenericError(
            "makensis not found. Please install NSIS (e.g., apt-get install nsis)".into(),
        )),
    }
}
