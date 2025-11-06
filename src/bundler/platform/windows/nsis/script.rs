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

    // Get all three required binaries
    let binaries = settings.binaries();

    // Find kodegen_install binary
    let kodegen_install = binaries
        .iter()
        .find(|b| b.name() == "kodegen_install")
        .ok_or_else(|| Error::GenericError("kodegen_install binary not found".into()))?;

    // Find kodegen binary (main MCP server)
    let kodegen = binaries
        .iter()
        .find(|b| b.name() == "kodegen")
        .ok_or_else(|| Error::GenericError("kodegen binary not found".into()))?;

    // Find kodegend binary (daemon)
    let kodegend = binaries
        .iter()
        .find(|b| b.name() == "kodegend")
        .ok_or_else(|| Error::GenericError("kodegend binary not found".into()))?;

    // Insert binary paths for template
    data.insert(
        "kodegen_install_path",
        settings.binary_path(kodegen_install).display().to_string(),
    );
    data.insert(
        "kodegen_path",
        settings.binary_path(kodegen).display().to_string(),
    );
    data.insert(
        "kodegend_path",
        settings.binary_path(kodegend).display().to_string(),
    );

    // Keep binary_name for backward compatibility with other template sections
    data.insert("binary_name", "kodegen".to_string());

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
