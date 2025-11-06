//! macOS DMG disk image creator.
#![allow(dead_code)] // Public API - items may be used by external consumers

//!
//! Creates professional drag-to-install DMG files using the native hdiutil tool.
//! The DMG includes the .app bundle and an Applications symlink for easy installation.
//!
//! # Architecture
//!
//! This module is organized into logical submodules:
//! - `creation` - Core DMG creation using hdiutil
//! - `customization` - DMG appearance customization (background, window size)
//! - `conversion` - Format conversion (UDRW → UDZO)

mod conversion;
mod creation;
mod customization;

use crate::bundler::{error::Result, settings::Settings, utils::fs};
use std::path::PathBuf;

// Re-export public functions from submodules
pub use conversion::convert_dmg_to_compressed;
pub use creation::{create_dmg, find_or_create_app_bundle, should_sign_dmg};
pub use customization::apply_dmg_customizations;

/// Bundle project as DMG disk image
///
/// # Process
/// 1. Find existing .app or create new one via app::bundle_project()
/// 2. Create temporary staging directory
/// 3. Copy .app into staging directory
/// 4. Sign and notarize the staged .app (Task 12 integration)
/// 5. Create Applications symlink for drag-to-install
/// 6. Generate DMG using hdiutil with UDZO compression
/// 7. Sign DMG if signing identity configured
/// 8. Clean up temporary files
///
/// # Arguments
/// * `settings` - Bundle configuration
/// * `runtime_identity` - Optional signing identity from TempKeychain (via APPLE_CERTIFICATE env var)
///
/// # Returns
/// Vector containing path to created DMG file.
///
/// # Example
/// ```no_run
/// # use std::path::PathBuf;
/// # type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
/// # struct Settings;
/// # fn bundle_project(settings: &Settings, runtime_identity: Option<&str>) -> Result<Vec<PathBuf>> { Ok(vec![]) }
/// # fn example() -> Result<()> {
/// # let settings = Settings;
/// let paths = bundle_project(&settings, None)?;
/// if !paths.is_empty() {
///     println!("Created DMG: {}", paths[0].display());
/// }
/// # Ok(())
/// # }
/// ```
pub async fn bundle_project(
    settings: &Settings,
    runtime_identity: Option<&str>,
) -> Result<Vec<PathBuf>> {
    log::info!("Creating DMG for {}", settings.product_name());

    // Step 1: Find or create .app bundle
    let app_bundle_path = find_or_create_app_bundle(settings, runtime_identity).await?;

    // Step 2: Prepare DMG output directory
    let output_dir = settings.project_out_directory().join("bundle/dmg");
    fs::create_dir_all(&output_dir, false).await?;

    // Step 3: Create DMG file
    let dmg_path = create_dmg(settings, &app_bundle_path, &output_dir, runtime_identity).await?;

    // Step 4: Apply customizations if configured
    let dmg_settings = &settings.bundle_settings().dmg;
    let needs_customization =
        dmg_settings.background.is_some() || dmg_settings.window_size.is_some();

    if needs_customization {
        apply_dmg_customizations(&dmg_path, settings).await?;
        convert_dmg_to_compressed(&dmg_path).await?;
    }

    // Step 5: Sign DMG if configured
    if should_sign_dmg(settings) {
        super::sign::sign_dmg(&dmg_path, settings).await?;
    }

    Ok(vec![dmg_path])
}
