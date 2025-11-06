//! Core DMG creation logic using hdiutil.
//!
//! Handles the fundamental DMG creation workflow including:
//! - Finding or creating the .app bundle
//! - Staging files in a temporary directory
//! - Creating Applications symlink
//! - Running hdiutil to generate the DMG

use crate::bundler::{
    error::{Context, ErrorExt, Result},
    settings::Settings,
    utils::fs,
};
use std::path::{Path, PathBuf};
use tokio::fs::remove_file;

/// Find existing .app bundle or create new one
///
/// # Logic
/// 1. Check if .app exists in expected location: `bundle/macos/{ProductName}.app`
/// 2. If found and is directory → use existing
/// 3. If not found → call `app::bundle_project()` to create it
///
/// # Returns
/// PathBuf to the .app bundle
pub async fn find_or_create_app_bundle(
    settings: &Settings,
    runtime_identity: Option<&str>,
) -> Result<PathBuf> {
    let app_name = format!("{}.app", settings.product_name());
    let expected_path = settings
        .project_out_directory()
        .join("bundle/macos")
        .join(&app_name);

    if expected_path.exists() && expected_path.is_dir() {
        log::debug!("Using existing .app bundle: {}", expected_path.display());
        return Ok(expected_path);
    }

    // Create .app bundle using existing app bundler
    log::info!("Creating .app bundle for DMG...");
    use super::super::app;
    let paths = app::bundle_project(settings, runtime_identity).await?;

    paths
        .into_iter()
        .next()
        .ok_or_else(|| crate::bundler::Error::GenericError("Failed to create .app bundle".into()))
}

/// Create DMG from .app bundle using hdiutil
///
/// # DMG Creation Steps
/// 1. Create temporary staging directory using tempfile crate
/// 2. Copy .app bundle to staging directory
/// 3. Sign and notarize the staged .app (before DMG creation)
/// 4. Create Applications symlink: `staging/Applications -> /Applications`
/// 5. Run hdiutil create with appropriate format
/// 6. Verify hdiutil succeeded
/// 7. Automatic cleanup (tempfile handles it)
///
/// # DMG Naming Convention
/// Format: `{ProductName}-{Version}.dmg`
/// Examples:
/// - `MyApp-1.0.0.dmg`
/// - `CoolTool-2.3.1.dmg`
///
/// # Returns
/// PathBuf to created DMG file
pub async fn create_dmg(
    settings: &Settings,
    app_bundle: &Path,
    output_dir: &Path,
    runtime_identity: Option<&str>,
) -> Result<PathBuf> {
    let dmg_name = format!(
        "{}-{}.dmg",
        settings.product_name(),
        settings.version_string()
    );
    let dmg_path = output_dir.join(&dmg_name);

    // Remove old DMG if exists
    if dmg_path.exists() {
        remove_file(&dmg_path).await?;
    }

    // Create temporary staging directory
    let temp_dir = tempfile::tempdir().map_err(|e| {
        crate::bundler::Error::GenericError(format!(
            "Failed to create temporary directory for DMG contents: {}",
            e
        ))
    })?;
    let staging_path = temp_dir.path();

    // Copy .app bundle to staging directory
    let app_name = app_bundle
        .file_name()
        .ok_or_else(|| crate::bundler::Error::GenericError("Invalid app bundle path".into()))?;
    let staged_app = staging_path.join(app_name);

    log::debug!("Copying .app to staging: {}", staged_app.display());
    fs::copy_dir(app_bundle, &staged_app)
        .await
        .with_context(|| {
            format!(
                "copying .app bundle to staging directory: {}",
                staged_app.display()
            )
        })?;

    // Sign and notarize the .app bundle BEFORE creating the DMG
    // This ensures the .app inside the DMG is properly signed and notarized
    if let Some(identity) = runtime_identity {
        super::super::sign::sign_app(&staged_app, identity, settings).await?;
    }

    if super::super::sign::should_notarize(settings).await {
        super::super::sign::notarize_app(&staged_app, settings).await?;
    }

    // Create Applications symlink for drag-to-install UX
    #[cfg(unix)]
    {
        let applications_link = staging_path.join("Applications");
        std::os::unix::fs::symlink("/Applications", &applications_link)
            .fs_context("creating Applications symlink", &applications_link)?;
    }

    // Determine if customization is needed
    let dmg_settings = &settings.bundle_settings().dmg;
    let needs_customization =
        dmg_settings.background.is_some() || dmg_settings.window_size.is_some();

    // Choose format: UDRW if customizing (so changes persist), UDZO if not
    let dmg_format = if needs_customization { "UDRW" } else { "UDZO" };

    log::info!("Creating DMG with format {}...", dmg_format);

    let staging_str = staging_path.to_str().ok_or_else(|| {
        crate::bundler::Error::GenericError(
            "Invalid staging path (contains non-UTF8 characters)".into(),
        )
    })?;

    let dmg_str = dmg_path.to_str().ok_or_else(|| {
        crate::bundler::Error::GenericError(
            "Invalid DMG path (contains non-UTF8 characters)".into(),
        )
    })?;

    let output = tokio::process::Command::new("hdiutil")
        .args([
            "create",
            "-volname",
            settings.product_name(),
            "-srcfolder",
            staging_str,
            "-ov", // Overwrite if exists
            "-format",
            dmg_format, // UDRW if customizing, UDZO if not
            dmg_str,
        ])
        .output()
        .await
        .map_err(|e| {
            crate::bundler::Error::GenericError(format!("Failed to execute hdiutil command: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::bundler::Error::GenericError(format!(
            "hdiutil failed: {}",
            stderr
        )));
    }

    log::info!("✓ Created {} DMG: {}", dmg_format, dmg_path.display());

    // tempfile automatically cleans up staging directory
    drop(temp_dir);

    Ok(dmg_path)
}

/// Check if DMG should be signed
///
/// Sign DMG when:
/// - ✅ `signing_identity` is configured in MacOsSettings
/// - ✅ Identity is NOT "-" (ad-hoc signature marker)
///
/// # Background
/// The "-" identity is Apple's marker for ad-hoc signatures (self-signing).
/// We skip external signing for ad-hoc signatures to avoid errors.
pub fn should_sign_dmg(settings: &Settings) -> bool {
    if let Some(identity) = &settings.bundle_settings().macos.signing_identity {
        identity != "-"
    } else {
        false
    }
}
