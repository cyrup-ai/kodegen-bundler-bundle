//! NSIS installer build execution.
//!
//! Compiles NSI scripts into Windows installer executables using makensis.

use crate::bundler::error::{Error, ErrorExt, Result};
use std::path::{Path, PathBuf};

/// Run makensis to compile NSI script into installer executable.
///
/// Executes the NSIS compiler (makensis) with appropriate arguments
/// to generate a Windows installer .exe from the NSI script.
///
/// # Arguments
/// - `nsis_path` - Path to NSIS installation directory containing makensis
/// - `nsi_path` - Path to the NSI script file to compile
/// - `output_path` - Path where the installer .exe should be created
///
/// # Platform-specific behavior
/// - Windows: Uses `makensis.exe` from the NSIS installation
/// - Unix: Uses system `makensis` command
pub async fn run_makensis(_nsis_path: &Path, nsi_path: &Path, output_path: &Path) -> Result<()> {
    log::info!("Running makensis...");

    let makensis = PathBuf::from("makensis");

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .fs_context("creating installer output directory", parent)?;
    }

    // Run makensis
    let status = tokio::process::Command::new(&makensis)
        .args([
            "-V3", // Verbosity level 3
            "-INPUTCHARSET",
            "UTF8",
            "-OUTPUTCHARSET",
            "UTF8",
            &format!("-DOUTPUT_FILE={}", output_path.display()),
            nsi_path
                .to_str()
                .ok_or_else(|| Error::GenericError("NSI path is not valid UTF-8".into()))?,
        ])
        .status()
        .await
        .map_err(|e| Error::CommandFailed {
            command: "makensis".to_string(),
            error: e,
        })?;

    if !status.success() {
        return Err(Error::GenericError("makensis compilation failed".into()));
    }

    Ok(())
}
