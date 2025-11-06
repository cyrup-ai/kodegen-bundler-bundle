//! Linux platform-specific settings.

use std::collections::HashMap;
use std::path::PathBuf;

/// Debian package (.deb) configuration.
///
/// Configures the creation of Debian packages for Ubuntu, Debian, and derivatives.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.linux.deb]
/// depends = ["libc6 (>= 2.31)", "libssl3"]
/// section = "devel"
/// priority = "optional"
/// ```
///
/// # Dependency Format
///
/// Dependencies follow Debian package syntax:
/// - `package-name` - Any version
/// - `package-name (>= 1.0)` - Minimum version
/// - `package-name (<< 2.0)` - Maximum version
///
/// # Desktop Integration
///
/// If a desktop file template is provided via `desktop_template`, it will be
/// installed to `/usr/share/applications/`.
///
/// # Maintainer Scripts
///
/// Lifecycle scripts are executed during installation/removal:
/// - `pre_install_script` - Before installation
/// - `post_install_script` - After installation  
/// - `pre_remove_script` - Before removal
/// - `post_remove_script` - After removal
///
/// # See Also
///
/// - [`RpmSettings`] - RPM package configuration
/// - [`AppImageSettings`] - AppImage configuration
#[derive(Clone, Debug, Default)]
pub struct DebianSettings {
    /// Package dependencies in Debian syntax.
    ///
    /// Example: `["libc6 (>= 2.31)", "libssl3"]`
    ///
    /// Default: None
    pub depends: Option<Vec<String>>,

    /// Package recommendations (optional dependencies).
    ///
    /// These packages enhance functionality but aren't required.
    ///
    /// Default: None
    pub recommends: Option<Vec<String>>,

    /// Virtual packages this package provides.
    ///
    /// Used for package alternatives and virtual package names.
    ///
    /// Default: None
    pub provides: Option<Vec<String>>,

    /// Packages that cannot be installed alongside this one.
    ///
    /// Default: None
    pub conflicts: Option<Vec<String>>,

    /// Packages this one replaces (for upgrades).
    ///
    /// Default: None
    pub replaces: Option<Vec<String>>,

    /// Custom files to add to package (destination -> source).
    ///
    /// Maps installation paths to source files.
    ///
    /// Default: Empty
    pub files: HashMap<PathBuf, PathBuf>,

    /// Path to custom `.desktop` file template.
    ///
    /// Will be installed to `/usr/share/applications/`.
    ///
    /// Default: None (auto-generated if not provided)
    pub desktop_template: Option<PathBuf>,

    /// Debian control file section.
    ///
    /// Common values: "utils", "devel", "admin", "net"
    ///
    /// Default: None (uses "utils")
    pub section: Option<String>,

    /// Package priority in Debian repository.
    ///
    /// Values: "required", "important", "standard", "optional", "extra"
    ///
    /// Default: None (uses "optional")
    pub priority: Option<String>,

    /// Path to Debian changelog file.
    ///
    /// Default: None (auto-generated)
    pub changelog: Option<PathBuf>,

    /// Pre-install script path (preinst).
    ///
    /// Executed before package installation.
    ///
    /// Default: None
    pub pre_install_script: Option<PathBuf>,

    /// Post-install script path (postinst).
    ///
    /// Executed after package installation.
    ///
    /// Default: None
    pub post_install_script: Option<PathBuf>,

    /// Pre-remove script path (prerm).
    ///
    /// Executed before package removal.
    ///
    /// Default: None
    pub pre_remove_script: Option<PathBuf>,

    /// Post-remove script path (postrm).
    ///
    /// Executed after package removal.
    ///
    /// Default: None
    pub post_remove_script: Option<PathBuf>,
}

/// RPM package (.rpm) configuration.
///
/// Configures the creation of RPM packages for Fedora, RHEL, CentOS, and derivatives.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.linux.rpm]
/// depends = ["glibc >= 2.31"]
/// release = "1"
/// compression = "zstd"
/// ```
///
/// # Compression Algorithms
///
/// Supported values for `compression`:
/// - `"gzip"` - Standard gzip compression
/// - `"xz"` - Better compression, slower
/// - `"zstd"` - Modern, balanced compression (recommended)
/// - `"bzip2"` - Legacy compression
///
/// # See Also
///
/// - [`DebianSettings`] - Debian package configuration
/// - [`AppImageSettings`] - AppImage configuration
#[derive(Clone, Debug)]
pub struct RpmSettings {
    /// Package dependencies in RPM syntax.
    ///
    /// Example: `["glibc >= 2.31", "openssl-libs"]`
    ///
    /// Default: None
    pub depends: Option<Vec<String>>,

    /// Package recommendations (weak dependencies).
    ///
    /// Default: None
    pub recommends: Option<Vec<String>>,

    /// Virtual packages this package provides.
    ///
    /// Default: None
    pub provides: Option<Vec<String>>,

    /// Packages that cannot be installed alongside this one.
    ///
    /// Default: None
    pub conflicts: Option<Vec<String>>,

    /// Packages this one obsoletes (supersedes).
    ///
    /// Default: None
    pub obsoletes: Option<Vec<String>>,

    /// Release number appended to version.
    ///
    /// Incremented for packaging changes without version bumps.
    ///
    /// Default: "1"
    pub release: String,

    /// Epoch number for version ordering.
    ///
    /// Used to force version ordering when normal comparison fails.
    /// Rarely needed.
    ///
    /// Default: 0
    pub epoch: u32,

    /// Custom files to add to package (destination -> source).
    ///
    /// Default: Empty
    pub files: HashMap<PathBuf, PathBuf>,

    /// Path to custom `.desktop` file template.
    ///
    /// Default: None (auto-generated)
    pub desktop_template: Option<PathBuf>,

    /// Pre-install script path (%pre).
    ///
    /// Default: None
    pub pre_install_script: Option<PathBuf>,

    /// Post-install script path (%post).
    ///
    /// Default: None
    pub post_install_script: Option<PathBuf>,

    /// Pre-remove script path (%preun).
    ///
    /// Default: None
    pub pre_remove_script: Option<PathBuf>,

    /// Post-remove script path (%postun).
    ///
    /// Default: None
    pub post_remove_script: Option<PathBuf>,

    /// Compression algorithm: "gzip", "xz", "zstd", "bzip2".
    ///
    /// Default: None (uses RPM default, typically "gzip")
    pub compression: Option<String>,
}

impl Default for RpmSettings {
    fn default() -> Self {
        Self {
            depends: None,
            recommends: None,
            provides: None,
            conflicts: None,
            obsoletes: None,
            release: "1".to_string(),
            epoch: 0,
            files: HashMap::new(),
            desktop_template: None,
            pre_install_script: None,
            post_install_script: None,
            pre_remove_script: None,
            post_remove_script: None,
            compression: None,
        }
    }
}

/// AppImage portable application configuration.
///
/// AppImage creates self-contained, portable executables for Linux that work
/// across distributions without installation.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.linux.appimage]
/// bundle_media_framework = true
/// ```
///
/// # Features
///
/// AppImages are portable executables that:
/// - Run on any Linux distribution
/// - Don't require installation or root privileges
/// - Bundle all dependencies internally
///
/// # See Also
///
/// - [`DebianSettings`] - Debian package configuration
/// - [`RpmSettings`] - RPM package configuration
#[derive(Clone, Debug, Default)]
pub struct AppImageSettings {
    /// Custom files to include (destination -> source).
    ///
    /// Default: Empty
    pub files: HashMap<PathBuf, PathBuf>,

    /// Bundle GStreamer media framework.
    ///
    /// Enable this if your application uses audio/video playback.
    ///
    /// Default: false
    pub bundle_media_framework: bool,

    /// Bundle xdg-open binary for opening URLs/files.
    ///
    /// Enable this if your application needs to open web browsers or files.
    ///
    /// Default: false
    pub bundle_xdg_open: bool,
}
