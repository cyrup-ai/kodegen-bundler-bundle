//! DMG format conversion utilities.
//!
//! Handles conversion between DMG formats, particularly:
//! - UDRW (read-write) to UDZO (compressed read-only)
//!
//! The conversion workflow is necessary because DMG customization requires
//! a read-write format, but the final distribution should use compressed format.

use crate::bundler::error::Result;
use std::path::Path;
use tokio::fs::{remove_file, rename};

/// Convert read-write DMG (UDRW) to compressed read-only (UDZO)
///
/// This must be done AFTER customizations are applied and the DMG is detached.
/// The conversion creates a new compressed DMG and replaces the original.
///
/// # Process
/// 1. Create temporary output path for compressed DMG
/// 2. Run hdiutil convert with UDZO format
/// 3. Remove original UDRW DMG
/// 4. Rename compressed DMG to original path
///
/// # Background
/// We cannot customize a UDZO DMG because it's compressed and read-only.
/// Changes made to a mounted UDZO with -readwrite are stored in a shadow
/// file which is discarded on detach. The correct workflow is:
/// UDRW → customize → detach → convert to UDZO.
pub async fn convert_dmg_to_compressed(dmg_path: &Path) -> Result<()> {
    log::info!("Converting DMG to compressed format...");

    let dmg_str = dmg_path.to_str().ok_or_else(|| {
        crate::bundler::Error::GenericError("DMG path contains non-UTF8 characters".into())
    })?;

    // Create temporary path for compressed DMG
    let compressed_path = dmg_path.with_extension("dmg.compressed");
    let compressed_str = compressed_path.to_str().ok_or_else(|| {
        crate::bundler::Error::GenericError(
            "Compressed DMG path contains non-UTF8 characters".into(),
        )
    })?;

    // Convert UDRW → UDZO
    let output = tokio::process::Command::new("hdiutil")
        .args(["convert", dmg_str, "-format", "UDZO", "-o", compressed_str])
        .output()
        .await
        .map_err(|e| {
            crate::bundler::Error::GenericError(format!("Failed to convert DMG: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::bundler::Error::GenericError(format!(
            "DMG conversion failed: {}",
            stderr
        )));
    }

    // Replace UDRW with UDZO
    remove_file(dmg_path).await?;
    rename(&compressed_path, dmg_path).await?;

    log::info!("✓ DMG converted to compressed UDZO format");

    Ok(())
}
