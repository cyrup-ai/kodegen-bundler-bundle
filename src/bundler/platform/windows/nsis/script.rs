//! NSIS installer script generation.
//!
//! Generates NSI installer scripts from templates using Handlebars,
//! with all necessary metadata, paths, and configuration settings.

use super::{template::NSI_TEMPLATE, utils};
use crate::bundler::{
    error::{Error, Result},
    settings::Settings,
};
use handlebars::Handlebars;
use std::path::Path;

/// Generate NSI installer script from template.
///
/// Uses handlebars to render template with settings data.
/// Writes output with UTF-8 BOM required by NSIS.
///
/// # Arguments
/// - `settings` - Bundler settings containing product metadata and paths
/// - `arch` - Target architecture string (e.g., "x64", "x86", "arm64")
/// - `output_dir` - Directory to write the generated .nsi file
///
/// # Returns
/// Path to the generated installer.nsi file
pub async fn generate_nsi_script(
    settings: &Settings,
    arch: &str,
    output_dir: &Path,
) -> Result<std::path::PathBuf> {
    let mut handlebars = Handlebars::new();
    handlebars.register_escape_fn(handlebars::no_escape);

    // Get all binaries (same pattern as Debian/RPM/AppImage bundlers)
    let binaries = settings.binaries();

    if binaries.is_empty() {
        return Err(Error::GenericError("No binaries found to bundle".into()));
    }

    // Collect all binary paths for template (with .exe extension for Windows)
    let binary_files: Vec<_> = binaries
        .iter()
        .map(|b| {
            let path = settings.binary_path(b);
            path.with_extension("exe").display().to_string()
        })
        .collect();

    // Get main binary name for shortcuts (find main binary or use first)
    let main_binary = binaries
        .iter()
        .find(|b| b.main())
        .or_else(|| binaries.first())
        .ok_or_else(|| Error::GenericError("No binaries found".into()))?;

    // Format version for NSIS VIProductVersion (requires exactly 4 parts)
    let version_nsis = utils::format_version_for_nsis(settings.version_string())?;

    // Publisher/manufacturer
    let publisher = settings
        .bundle_settings()
        .publisher
        .as_deref()
        .unwrap_or("Unknown Publisher");

    // Install directory based on install mode
    let install_dir = match settings.bundle_settings().windows.nsis.install_mode {
        crate::bundler::settings::NSISInstallerMode::PerMachine => {
            format!("$PROGRAMFILES64\\{}", settings.product_name())
        }
        crate::bundler::settings::NSISInstallerMode::CurrentUser => {
            format!("$LOCALAPPDATA\\{}", settings.product_name())
        }
        crate::bundler::settings::NSISInstallerMode::Both => {
            format!("$PROGRAMFILES64\\{}", settings.product_name())
        }
    };

    // Custom branding images
    let nsis_settings = &settings.bundle_settings().windows.nsis;

    // Build template data with mixed types (strings and arrays)
    let mut data = serde_json::json!({
        "product_name": settings.product_name(),
        "version": settings.version_string(),
        "version_nsis": version_nsis,
        "arch": arch,
        "publisher": publisher,
        "binary_files": binary_files,
        "binary_name": main_binary.name(),
        "install_dir": install_dir,
        "install_mode": utils::map_install_mode(settings.bundle_settings().windows.nsis.install_mode),
        "compression": utils::map_compression(settings.bundle_settings().windows.nsis.compression),
    });

    // Add optional branding images if present
    if let Some(header) = &nsis_settings.header_image {
        data["header_image"] = serde_json::json!(header.display().to_string());
    }

    if let Some(sidebar) = &nsis_settings.sidebar_image {
        data["sidebar_image"] = serde_json::json!(sidebar.display().to_string());
    }

    if let Some(icon) = &nsis_settings.installer_icon {
        data["installer_icon"] = serde_json::json!(icon.display().to_string());
    }

    // Render template
    handlebars
        .register_template_string("installer.nsi", NSI_TEMPLATE)
        .map_err(|e| Error::GenericError(format!("failed to register NSI template: {}", e)))?;

    let nsi_content = handlebars
        .render("installer.nsi", &data)
        .map_err(|e| Error::GenericError(format!("failed to render NSI template: {}", e)))?;

    // Write with UTF-8 BOM
    let nsi_path = output_dir.join("installer.nsi");
    utils::write_utf8_bom(&nsi_path, &nsi_content).await?;

    Ok(nsi_path)
}
