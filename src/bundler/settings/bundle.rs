//! Bundle configuration and binary definitions.

use super::{
    AppImageSettings, DebianSettings, DmgSettings, MacOsSettings, RpmSettings, WindowsSettings,
};
use std::path::PathBuf;

/// Platform-specific application category settings.
///
/// Different platforms require different category formats:
/// - Linux: freedesktop.org categories (e.g., "Development", "Utility")
/// - macOS: LSApplicationCategoryType (e.g., "public.app-category.developer-tools")
/// - Windows: Optional custom category
///
/// # Example
///
/// ```toml
/// [package.metadata.bundle.category]
/// linux = "Development"
/// macos = "public.app-category.developer-tools"
/// ```
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CategorySettings {
    /// Linux category (freedesktop.org Desktop Entry Specification).
    ///
    /// Valid values: AudioVideo, Audio, Video, Development, Education, Game,
    /// Graphics, Network, Office, Settings, Utility
    #[serde(default)]
    pub linux: Option<String>,

    /// macOS category (LSApplicationCategoryType).
    ///
    /// Example: "public.app-category.developer-tools"
    #[serde(default)]
    pub macos: Option<String>,

    /// Windows category (optional).
    #[serde(default)]
    pub windows: Option<String>,
}

/// Bundle configuration for all platforms.
///
/// Central configuration structure containing metadata and platform-specific settings.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle]
/// identifier = "com.example.app"
/// publisher = "Example Inc."
/// icon = ["assets/icon.png"]
/// resources = ["config/**/*"]
/// category = "Utility"
/// ```
///
/// # See Also
///
/// - [`DebianSettings`] - Debian package configuration
/// - [`RpmSettings`] - RPM package configuration
/// - [`MacOsSettings`] - macOS app bundle configuration
/// - [`WindowsSettings`] - Windows installer configuration
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct BundleSettings {
    /// Bundle identifier in reverse domain notation.
    ///
    /// Example: "com.example.app", "ai.kodegen.app"
    ///
    /// Required for macOS and some Linux desktop integrations.
    ///
    /// Default: None
    #[serde(default)]
    pub identifier: Option<String>,

    /// Publisher/company name.
    ///
    /// Default: None
    #[serde(default)]
    pub publisher: Option<String>,

    /// Icon file paths (PNG recommended).
    ///
    /// Provide multiple sizes for best quality:
    /// `["icon-32.png", "icon-128.png", "icon-512.png"]`
    ///
    /// Auto-converted to platform-specific formats (ICNS, ICO).
    ///
    /// Default: None
    #[serde(default)]
    pub icon: Option<Vec<PathBuf>>,

    /// Pre-made ICNS file for macOS (optional).
    ///
    /// If provided, this file will be copied directly instead of generating from PNGs.
    ///
    /// Default: None
    #[serde(default)]
    pub icns: Option<PathBuf>,

    /// Pre-made ICO file for Windows (optional).
    ///
    /// If provided, this file will be copied directly instead of generating from PNGs.
    ///
    /// Default: None
    #[serde(default)]
    pub ico: Option<PathBuf>,

    /// Resource glob patterns to bundle.
    ///
    /// Example: `["config/**/*", "templates/**/*"]`
    ///
    /// Default: None
    #[serde(default)]
    pub resources: Option<Vec<String>>,

    /// Copyright notice string.
    ///
    /// Example: "Copyright © 2024 Example Inc."
    ///
    /// Default: None
    #[serde(default)]
    pub copyright: Option<String>,

    /// Application category (platform-specific).
    ///
    /// Configure in Cargo.toml:
    /// ```toml
    /// [package.metadata.bundle.category]
    /// linux = "Development"
    /// macos = "public.app-category.developer-tools"
    /// ```
    ///
    /// Default: None
    #[serde(default)]
    pub category: Option<CategorySettings>,

    /// Short description (one line).
    ///
    /// Used in package managers and installer summaries.
    ///
    /// Default: None
    #[serde(default)]
    pub short_description: Option<String>,

    /// Long description (multiple paragraphs).
    ///
    /// Used in package details and documentation.
    ///
    /// Default: None
    #[serde(default)]
    pub long_description: Option<String>,

    /// External binaries to bundle.
    ///
    /// List of binary names (without path). Each must have a platform-specific
    /// variant: `binary-{target}` or `binary-{target}.exe`
    ///
    /// Example: `["helper"]` expects `helper-x86_64-unknown-linux-gnu`, etc.
    ///
    /// Default: None
    #[serde(default)]
    pub external_bin: Option<Vec<String>>,

    /// Debian-specific settings.
    ///
    /// See [`DebianSettings`] for details.
    #[serde(default)]
    pub deb: DebianSettings,

    /// RPM-specific settings.
    ///
    /// See [`RpmSettings`] for details.
    #[serde(default)]
    pub rpm: RpmSettings,

    /// AppImage-specific settings.
    ///
    /// See [`AppImageSettings`] for details.
    #[serde(default)]
    pub appimage: AppImageSettings,

    /// macOS-specific settings.
    ///
    /// See [`MacOsSettings`] for details.
    #[serde(default)]
    pub macos: MacOsSettings,

    /// DMG-specific settings.
    ///
    /// See [`DmgSettings`] for details.
    #[serde(default)]
    pub dmg: DmgSettings,

    /// Windows-specific settings.
    ///
    /// See [`WindowsSettings`] for details.
    #[serde(default)]
    pub windows: WindowsSettings,
}

/// A binary to bundle into the installer.
///
/// Represents an executable to include in the bundle. Multiple binaries can be
/// bundled, but typically one is marked as the main executable.
///
/// # Examples
///
/// ```no_run
/// use kodegen_bundler_release::bundler::BundleBinary;
///
/// let main_binary = BundleBinary::new("myapp".into(), true);
/// let helper = BundleBinary::new("myapp-helper".into(), false);
/// ```
#[derive(Clone, Debug)]
pub struct BundleBinary {
    name: String,
    main: bool,
    src_path: Option<String>,
}

impl BundleBinary {
    /// Creates a new bundle binary.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary name (without extension)
    /// * `main` - Whether this is the main executable
    pub fn new(name: String, main: bool) -> Self {
        Self {
            name,
            main,
            src_path: None,
        }
    }

    /// Creates a new bundle binary with source path.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary name (without extension)
    /// * `main` - Whether this is the main executable
    /// * `src_path` - Optional path to binary source
    pub fn with_path(name: String, main: bool, src_path: Option<String>) -> Self {
        Self {
            name,
            src_path,
            main,
        }
    }

    /// Mark the binary as the main executable.
    ///
    /// The main executable is used for desktop shortcuts and start menu entries.
    pub fn set_main(&mut self, main: bool) {
        self.main = main;
    }

    /// Sets the binary name.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Sets the source path of the binary.
    ///
    /// Returns self for method chaining.
    pub fn set_src_path(mut self, src_path: Option<String>) -> Self {
        self.src_path = src_path;
        self
    }

    /// Returns whether this is the main executable.
    pub fn main(&self) -> bool {
        self.main
    }

    /// Returns the binary name (without extension).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the binary source path if set.
    pub fn src_path(&self) -> Option<&String> {
        self.src_path.as_ref()
    }
}
