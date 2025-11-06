//! Windows NSIS installer creation.
//!
//! Creates lightweight, fast Windows installers using NSIS (Nullsoft Scriptable Install System).
//! Supports Modern UI, multiple architectures, compression algorithms, and install modes.
//!
//! # Module Organization
//!
//! - `template` - NSI script template constants
//! - `toolset` - NSIS toolset download and location
//! - `script` - NSI script generation from templates
//! - `build` - makensis execution and compilation
//! - `utils` - Helper functions (architecture mapping, version formatting, etc.)

mod build;
mod script;
mod template;
mod toolset;
mod utils;

use super::sign;
use crate::bundler::{
    error::{Context, ErrorExt, Result},
    settings::Settings,
};
use std::path::PathBuf;

/// Bundle project as NSIS installer.
///
/// Creates a Windows .exe installer with Modern UI wizard interface.
/// Downloads NSIS toolset on Windows, uses system makensis on Linux/macOS.
///
/// # Process
///
/// 1. Acquire NSIS toolset (download on Windows, locate on Unix)
/// 2. Map target architecture to NSIS arch string
/// 3. Create output directory structure
/// 4. Generate NSI script from template with settings
/// 5. Compile NSI script using makensis
/// 6. Sign installer if configured
///
/// # Returns
///
/// Vector containing the path to the generated installer .exe file
pub async fn bundle_project(settings: &Settings) -> Result<Vec<PathBuf>> {
    log::info!("Building NSIS installer for {}", settings.product_name());

    // Get NSIS toolset
    let nsis_path = toolset::get_nsis_toolset().await?;

    // Map architecture
    let arch = utils::map_arch(settings.binary_arch())?;

    // Create output directory
    let output_dir = settings
        .project_out_directory()
        .join("bundle/nsis")
        .join(arch);
    tokio::fs::create_dir_all(&output_dir)
        .await
        .fs_context("creating NSIS output directory", &output_dir)?;

    // Generate NSI script
    let nsi_path = script::generate_nsi_script(settings, arch, &output_dir).await?;

    // Create installer name
    let installer_name = format!(
        "{}_{}_{}-setup.exe",
        settings.product_name(),
        settings.version_string(),
        arch
    );
    let installer_path = settings
        .project_out_directory()
        .join("bundle/nsis")
        .join(&installer_name);

    // Run makensis
    build::run_makensis(&nsis_path, &nsi_path, &installer_path).await?;

    // Sign the installer if configured
    if sign::should_sign(settings) {
        sign::sign_file(&installer_path, settings)
            .await
            .context("signing NSIS installer")?;
    }

    log::info!("✓ Created NSIS installer: {}", installer_path.display());

    Ok(vec![installer_path])
}
