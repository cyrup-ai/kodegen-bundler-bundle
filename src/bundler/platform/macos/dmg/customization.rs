//! DMG appearance customization using AppleScript.
//!
//! Handles all DMG customization features including:
//! - Mounting DMG in read-write mode
//! - Copying background images
//! - Running AppleScript to set window properties
//! - Detaching DMG after customization

use crate::bundler::{error::Result, settings::Settings, utils::fs};
use std::path::{Path, PathBuf};
use tokio::fs::copy;
use tokio::time::Duration;

/// Apply DMG customizations (background image and window size)
///
/// # Process
/// 1. Mount DMG in read-write mode
/// 2. Copy background image to .background folder (if configured)
/// 3. Run AppleScript to customize window appearance
/// 4. Wait for .DS_Store file to be created
/// 5. Detach DMG
///
/// # Background
/// DMG appearance customization requires:
/// - Mounting the DMG to modify its .DS_Store file
/// - Using AppleScript to set Finder window properties
/// - The .DS_Store file persists these settings when DMG is unmounted
pub async fn apply_dmg_customizations(dmg_path: &Path, settings: &Settings) -> Result<()> {
    log::info!("Applying DMG customizations...");

    let dmg_settings = &settings.bundle_settings().dmg;

    // Step 1: Mount DMG in read-write mode
    let volume_name = settings.product_name();
    let mount_point = mount_dmg_rw(dmg_path, volume_name).await?;

    // Step 2: Copy background image if configured
    if let Some(bg_path) = &dmg_settings.background {
        let bg_dir = mount_point.join(".background");
        fs::create_dir_all(&bg_dir, false).await?;

        let bg_filename = bg_path.file_name().ok_or_else(|| {
            crate::bundler::Error::GenericError("Invalid background image path".into())
        })?;

        let dest_bg = bg_dir.join(bg_filename);
        copy(bg_path, &dest_bg).await?;

        log::debug!("Copied background image to {}", dest_bg.display());
    }

    // Step 3: Run AppleScript to customize window
    let window_size = dmg_settings.window_size.unwrap_or((600, 400));
    let has_background = dmg_settings.background.is_some();

    run_dmg_applescript(volume_name, settings, window_size, has_background).await?;

    // Step 4: Detach DMG
    detach_dmg(volume_name).await?;

    log::info!("✓ DMG customizations applied");

    Ok(())
}

/// Mount DMG in read-write mode
///
/// Returns the mount point path
async fn mount_dmg_rw(dmg_path: &Path, volume_name: &str) -> Result<PathBuf> {
    log::debug!("Mounting DMG for customization...");

    let dmg_str = dmg_path.to_str().ok_or_else(|| {
        crate::bundler::Error::GenericError("DMG path contains non-UTF8 characters".into())
    })?;

    let output = tokio::process::Command::new("hdiutil")
        .args(["attach", dmg_str, "-readwrite", "-noverify", "-nobrowse"])
        .output()
        .await
        .map_err(|e| crate::bundler::Error::GenericError(format!("Failed to mount DMG: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::bundler::Error::GenericError(format!(
            "Failed to mount DMG: {}",
            stderr
        )));
    }

    // Mount point is /Volumes/{volume_name}
    let mount_point = PathBuf::from(format!("/Volumes/{}", volume_name));

    // Wait for mount to be ready
    let max_retries = 10;
    for i in 0..max_retries {
        if mount_point.exists() {
            log::debug!("DMG mounted at {}", mount_point.display());
            return Ok(mount_point);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        if i == max_retries - 1 {
            return Err(crate::bundler::Error::GenericError(format!(
                "DMG mount point not found after {} retries",
                max_retries
            )));
        }
    }

    Ok(mount_point)
}

/// Escape special characters for AppleScript string literals
///
/// Escapes backslashes and double quotes to prevent script injection
/// and syntax errors when product names contain special characters.
///
/// # Examples
/// ```
/// # fn escape_applescript_string(s: &str) -> String {
/// #     s.replace('\\', r"\\").replace('"', r#"\""#)
/// # }
/// assert_eq!(escape_applescript_string("My\"App"), "My\\\"App");
/// assert_eq!(escape_applescript_string("Path\\File"), "Path\\\\File");
/// ```
fn escape_applescript_string(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', r#"\""#)
}

/// Run AppleScript to customize DMG window appearance
async fn run_dmg_applescript(
    volume_name: &str,
    settings: &Settings,
    window_size: (u32, u32),
    has_background: bool,
) -> Result<()> {
    log::debug!("Running AppleScript to customize DMG window...");

    let app_name = format!("{}.app", settings.product_name());
    let (width, height) = window_size;

    // Escape strings for safe AppleScript interpolation
    let escaped_volume = escape_applescript_string(volume_name);
    let escaped_app = escape_applescript_string(&app_name);

    // Extract and escape background filename
    let escaped_bg_filename = if has_background {
        let bg_filename = settings
            .bundle_settings()
            .dmg
            .background
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("background.png");
        escape_applescript_string(bg_filename)
    } else {
        String::new()
    };

    // Build AppleScript (use escaped variables)
    let script = format!(
        r#"
        tell application "Finder"
            tell disk "{volume_name}"
                open
                set current view of container window to icon view
                set toolbar visible of container window to false
                set statusbar visible of container window to false
                set bounds of container window to {{100, 100, {right}, {bottom}}}
                set viewOptions to icon view options of container window
                set arrangement of viewOptions to not arranged
                set icon size of viewOptions to 72
                {background_clause}
                set position of item "{app_name}" to {{180, 170}}
                set position of item "Applications" to {{480, 170}}
                close
                open
                update without registering applications
                delay 2
            end tell
        end tell
        "#,
        volume_name = escaped_volume,
        right = 100 + width,
        bottom = 100 + height,
        app_name = escaped_app,
        background_clause = if has_background {
            format!(
                r#"set background picture of viewOptions to file ".background:{bg_filename}""#,
                bg_filename = escaped_bg_filename
            )
        } else {
            String::new()
        }
    );

    let output = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .await
        .map_err(|e| {
            crate::bundler::Error::GenericError(format!("Failed to run AppleScript: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("AppleScript execution had issues: {}", stderr);
        // Don't fail - appearance customization is non-critical
    }

    Ok(())
}

/// Detach (unmount) DMG
async fn detach_dmg(volume_name: &str) -> Result<()> {
    log::debug!("Detaching DMG...");

    let mount_point = format!("/Volumes/{}", volume_name);

    // Wait for .DS_Store to be written
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let output = tokio::process::Command::new("hdiutil")
        .args(["detach", &mount_point])
        .output()
        .await
        .map_err(|e| crate::bundler::Error::GenericError(format!("Failed to detach DMG: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("DMG detach had issues: {}", stderr);
        // Try force detach
        tokio::process::Command::new("hdiutil")
            .args(["detach", &mount_point, "-force"])
            .output()
            .await
            .ok();
    }

    Ok(())
}
