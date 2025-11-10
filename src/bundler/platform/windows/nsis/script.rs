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
use std::{collections::BTreeMap, path::Path};

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

    let mut data = BTreeMap::new();

    // Basic metadata
    data.insert("product_name", settings.product_name().to_string());
    data.insert("version", settings.version_string().to_string());

    // Format version for NSIS VIProductVersion (requires exactly 4 parts)
    let version_nsis = utils::format_version_for_nsis(settings.version_string())?;
    data.insert("version_nsis", version_nsis);

    data.insert("arch", arch.to_string());

    // Publisher/manufacturer
    let publisher = settings
        .bundle_settings()
        .publisher
        .as_deref()
        .unwrap_or("Unknown Publisher");
    data.insert("publisher", publisher.to_string());

    // Get all binaries (same pattern as Debian/RPM/AppImage bundlers)
    let binaries = settings.binaries();

    if binaries.is_empty() {
        return Err(Error::GenericError("No binaries found to bundle".into()));
    }

    // Collect all binary paths for template
    let binary_files: Vec<_> = binaries
        .iter()
        .map(|b| settings.binary_path(b).display().to_string())
        .collect();
    data.insert("binary_files", binary_files);

    // Get main binary name for shortcuts (find main binary or use first)
    let main_binary = binaries
        .iter()
        .find(|b| b.main())
        .or_else(|| binaries.first())
        .ok_or_else(|| Error::GenericError("No binaries found".into()))?;

    data.insert("binary_name", main_binary.name().to_string());

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
    data.insert("install_dir", install_dir);

    // Installer settings
    data.insert(
        "install_mode",
        utils::map_install_mode(settings.bundle_settings().windows.nsis.install_mode).to_string(),
    );
    data.insert(
        "compression",
        utils::map_compression(settings.bundle_settings().windows.nsis.compression).to_string(),
    );

    // Custom branding images
    let nsis_settings = &settings.bundle_settings().windows.nsis;

    if let Some(header) = &nsis_settings.header_image {
        data.insert("header_image", header.display().to_string());
    }

    if let Some(sidebar) = &nsis_settings.sidebar_image {
        data.insert("sidebar_image", sidebar.display().to_string());
    }

    if let Some(icon) = &nsis_settings.installer_icon {
        data.insert("installer_icon", icon.display().to_string());
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
