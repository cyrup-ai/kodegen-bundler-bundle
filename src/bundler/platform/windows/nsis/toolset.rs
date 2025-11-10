//! NSIS toolset acquisition and management.
//!
//! Locates system-installed makensis binary on Linux/macOS.

use crate::bundler::error::{Error, Result};
use std::path::PathBuf;

/// Get NSIS toolset.
///
/// Locates system-installed makensis on Linux/macOS.
///
/// Returns the path to the NSIS directory containing makensis executable.
pub async fn get_nsis_toolset() -> Result<PathBuf> {
    get_nsis_unix()
}

/// Locate system-installed makensis on Unix systems.
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
