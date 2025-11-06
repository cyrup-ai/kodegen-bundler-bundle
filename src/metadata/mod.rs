//! Metadata and binary discovery from single Cargo.toml

use crate::error::{CliError, BundlerError, Result};
use crate::bundler::BundleSettings;
use std::path::Path;

/// Package metadata extracted from Cargo.toml
#[allow(dead_code)] // Public API - preserved for external consumers
pub struct PackageMetadata {
    /// Package name from Cargo.toml
    pub name: String,

    /// Package description from Cargo.toml
    pub description: String,

    /// Package version from Cargo.toml (e.g., "0.1.0")
    pub version: String,

    /// List of package authors from Cargo.toml
    pub authors: Vec<String>,

    /// SPDX license identifier (e.g., "Apache-2.0 OR MIT")
    pub license: Option<String>,

    /// Homepage URL if specified in Cargo.toml
    pub homepage: Option<String>,
}

/// Complete manifest data from Cargo.toml
#[allow(dead_code)] // Public API - preserved for external consumers
pub struct CargoManifest {
    /// Package metadata ([package] section)
    pub metadata: PackageMetadata,

    /// Primary binary name (from [[bin]] or package.name)
    pub binary_name: String,

    /// Bundle settings (from [package.metadata.bundle] section + asset discovery)
    pub bundle_settings: BundleSettings,
}

/// Load complete manifest from Cargo.toml (single read + parse)
///
/// This function reads and parses Cargo.toml exactly once, then extracts
/// both metadata and binary name from the parsed TOML value.
///
/// ## Performance
/// Replaces two separate read+parse operations with one atomic operation.
///
/// ## Pattern
/// Follows the same optimization used in workspace/analyzer.rs:145-157
/// where root Cargo.toml is parsed once and passed to multiple functions.
#[allow(dead_code)] // Public API - preserved for external consumers
pub fn load_manifest(cargo_toml_path: &Path) -> Result<CargoManifest> {
    // Step 1: Read file once
    let manifest = std::fs::read_to_string(cargo_toml_path).map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "read_cargo_toml".to_string(),
            reason: format!("Failed to read {}: {}", cargo_toml_path.display(), e),
        })
    })?;

    // Step 2: Parse TOML once
    let toml_value: toml::Value = toml::from_str(&manifest).map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "parse_cargo_toml".to_string(),
            reason: format!("Failed to parse Cargo.toml: {}", e),
        })
    })?;

    let package = toml_value.get("package").ok_or_else(|| {
        BundlerError::Cli(CliError::InvalidArguments {
            reason: "No [package] section in Cargo.toml".to_string(),
        })
    })?;

    // Step 3: Extract metadata from parsed TOML (no additional I/O)
    let metadata = PackageMetadata {
        name: package
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BundlerError::Cli(CliError::InvalidArguments {
                    reason: "Missing 'name' in [package]".to_string(),
                })
            })?
            .to_string(),

        description: package
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Rust application")
            .to_string(),

        version: package
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BundlerError::Cli(CliError::InvalidArguments {
                    reason: "Missing 'version' in [package]".to_string(),
                })
            })?
            .to_string(),

        authors: package
            .get("authors")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),

        license: package
            .get("license")
            .and_then(|v| v.as_str())
            .map(String::from),

        homepage: package
            .get("homepage")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    // Step 4: Discover binary name from parsed TOML (no additional I/O)
    // Try [[bin]] section first
    let binary_name = toml_value
        .get("bin")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            // Fallback to package name
            package
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .ok_or_else(|| {
            BundlerError::Cli(CliError::InvalidArguments {
                reason: "No binary found in Cargo.toml".to_string(),
            })
        })?;

    // Step 5: Parse bundle settings from [package.metadata.bundle] section
    let cargo_dir = cargo_toml_path.parent().ok_or_else(|| {
        BundlerError::Cli(CliError::InvalidArguments {
            reason: "Invalid Cargo.toml path".to_string(),
        })
    })?;

    let mut bundle_settings = parse_bundle_settings(&toml_value)?;

    // Step 6: Discover assets from conventional location
    discover_bundle_assets(cargo_dir, &mut bundle_settings)?;

    Ok(CargoManifest {
        metadata,
        binary_name,
        bundle_settings,
    })
}

/// Parse bundle settings from [package.metadata.bundle] section
///
/// Extracts configuration for platform-specific bundling including required
/// bundle identifier for macOS.
fn parse_bundle_settings(toml_value: &toml::Value) -> Result<BundleSettings> {
    let mut settings = BundleSettings::default();

    if let Some(metadata) = toml_value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("bundle"))
    {
        // Parse bundle identifier (REQUIRED for macOS)
        settings.identifier = metadata
            .get("identifier")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse optional fields
        settings.publisher = metadata
            .get("publisher")
            .and_then(|v| v.as_str())
            .map(String::from);

        settings.category = metadata
            .get("category")
            .and_then(|v| v.as_str())
            .map(String::from);

        settings.copyright = metadata
            .get("copyright")
            .and_then(|v| v.as_str())
            .map(String::from);

        settings.short_description = metadata
            .get("short_description")
            .and_then(|v| v.as_str())
            .map(String::from);

        settings.long_description = metadata
            .get("long_description")
            .and_then(|v| v.as_str())
            .map(String::from);
    }

    Ok(settings)
}

/// Discover bundle assets from conventional directory structure
///
/// Scans for REQUIRED platform-specific icon files in assets/img/:
/// - icon.icns (macOS)
/// - icon.ico (Windows)
/// - icon_*x*.png (Linux - multiple sizes including @2x variants)
///
/// Files are only added if they exist. Platform-specific bundlers will
/// error if their required icon is missing.
fn discover_bundle_assets(package_root: &Path, settings: &mut BundleSettings) -> Result<()> {
    let assets_dir = package_root.join("assets").join("img");

    if !assets_dir.exists() {
        log::warn!("Assets directory not found: {}", assets_dir.display());
        log::warn!("Expected platform-specific icons in assets/img/");
        return Ok(());
    }

    let mut icons = Vec::new();

    // Platform-specific icons (macOS, Windows)
    let platform_icons = [
        ("icon.icns", "macOS"),
        ("icon.ico", "Windows"),
    ];

    for (filename, platform) in platform_icons {
        let icon_path = assets_dir.join(filename);
        if icon_path.exists() {
            log::info!("Found {} icon: {}", platform, icon_path.display());
            icons.push(icon_path);
        } else {
            log::debug!("{} icon not found: {}", platform, icon_path.display());
        }
    }

    // Linux PNG icons (multiple sizes + @2x variants)
    let linux_icon_sizes = [
        "icon_16x16.png",
        "icon_16x16@2x.png",
        "icon_32x32.png",
        "icon_32x32@2x.png",
        "icon_128x128.png",
        "icon_128x128@2x.png",
        "icon_256x256.png",
        "icon_256x256@2x.png",
        "icon_512x512.png",
        "icon_512x512@2x.png",
    ];

    let mut linux_icons_found = 0;
    for filename in linux_icon_sizes {
        let icon_path = assets_dir.join(filename);
        if icon_path.exists() {
            log::debug!("Found Linux icon: {}", filename);
            icons.push(icon_path);
            linux_icons_found += 1;
        }
    }

    if linux_icons_found > 0 {
        log::info!("Found {} Linux PNG icons", linux_icons_found);
    } else {
        log::debug!("No Linux PNG icons found");
    }

    if !icons.is_empty() {
        let icon_count = icons.len();
        settings.icon = Some(icons);
        log::info!("Discovered {} total icon files", icon_count);
    } else {
        log::warn!("No icon files found in assets/img/");
    }

    Ok(())
}
