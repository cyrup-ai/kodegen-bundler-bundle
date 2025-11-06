//! Windows platform-specific settings.

use std::path::PathBuf;

/// Windows installer configuration.
///
/// Configures Windows installers (MSI via WiX, EXE via NSIS) with optional
/// Authenticode code signing.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.windows]
/// cert_path = "cert.pem"
/// key_path = "key.pem"
/// timestamp_url = "http://timestamp.digicert.com"
/// ```
///
/// # Code Signing
///
/// See [`kodegen_sign`](../../sign/index.html) for Authenticode signing setup
/// using osslsigncode.
///
/// # See Also
///
/// - [`WixSettings`] - WiX MSI installer configuration
/// - [`NsisSettings`] - NSIS installer configuration
#[derive(Clone, Debug, Default)]
pub struct WindowsSettings {
    // === Signing Configuration ===
    /// Path to certificate file (.pem, .crt, .pfx).
    ///
    /// For PKCS#12 (.pfx), also set `password`.
    ///
    /// Default: None (unsigned)
    pub cert_path: Option<PathBuf>,

    /// Path to private key file (.pem, .key).
    ///
    /// Not needed for PKCS#12 (.pfx) files which contain both cert and key.
    ///
    /// Default: None
    pub key_path: Option<PathBuf>,

    /// Password for encrypted key or PKCS#12 file.
    ///
    /// Default: None
    pub password: Option<String>,

    /// Timestamp server URL for signature timestamping.
    ///
    /// Recommended: "http://timestamp.digicert.com"
    ///
    /// Default: None (uses default timestamp server)
    pub timestamp_url: Option<String>,

    // === Legacy/Alternative Fields ===
    /// Custom sign command for alternative signing tools.
    ///
    /// Example: "signtool sign /sha1 ABC123... %1"
    ///
    /// Default: None (uses osslsigncode)
    pub sign_command: Option<String>,

    // === Installer Settings ===
    /// WiX MSI installer settings.
    ///
    /// See [`WixSettings`] for details.
    pub wix: WixSettings,

    /// NSIS EXE installer settings.
    ///
    /// See [`NsisSettings`] for details.
    pub nsis: NsisSettings,
}

/// WiX MSI installer configuration.
///
/// WiX creates professional Windows Installer (.msi) packages.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.windows.wix]
/// language = ["en-US"]
/// license = "LICENSE.rtf"
/// ```
///
/// # See Also
///
/// - [`WindowsSettings`] - Windows installer configuration
/// - [`NsisSettings`] - NSIS installer configuration
#[derive(Clone, Debug, Default)]
pub struct WixSettings {
    /// Supported installer languages.
    ///
    /// Example: `["en-US", "de-DE", "fr-FR"]`
    ///
    /// Default: Empty (uses "en-US")
    pub language: Vec<String>,

    /// Path to custom WiX template (.wxs file).
    ///
    /// Default: None (uses built-in template)
    pub template: Option<PathBuf>,

    /// Paths to WiX fragment files to include.
    ///
    /// Default: Empty
    pub fragment_paths: Vec<PathBuf>,

    /// Component group references to include.
    ///
    /// Default: Empty
    pub component_group_refs: Vec<String>,

    /// Component references to include.
    ///
    /// Default: Empty
    pub component_refs: Vec<String>,

    /// Feature group references to include.
    ///
    /// Default: Empty
    pub feature_group_refs: Vec<String>,

    /// Feature references to include.
    ///
    /// Default: Empty
    pub feature_refs: Vec<String>,

    /// Merge module (.msm) references to include.
    ///
    /// Default: Empty
    pub merge_refs: Vec<String>,

    /// Skip WebView2 runtime installation.
    ///
    /// Set to true if your app doesn't use WebView2.
    ///
    /// Default: false
    pub skip_webview_install: bool,

    /// Path to license file (.rtf format required).
    ///
    /// Shown during installation.
    ///
    /// Default: None
    pub license: Option<PathBuf>,

    /// Enable elevated update task for automatic updates.
    ///
    /// Default: false
    pub enable_elevated_update_task: bool,

    /// Path to banner image (493×58 pixels).
    ///
    /// Shown at top of installer dialogs.
    ///
    /// Default: None
    pub banner_path: Option<PathBuf>,

    /// Path to dialog image (493×312 pixels).
    ///
    /// Shown on installer welcome screen.
    ///
    /// Default: None
    pub dialog_image_path: Option<PathBuf>,
}

/// NSIS installer mode (installation scope).
///
/// Determines whether the installer installs for the current user only,
/// all users (requires admin), or lets the user choose.
///
/// # Configuration
///
/// ```toml
/// [package.metadata.bundle.windows.nsis]
/// installer_mode = "perMachine"  # or "currentUser" or "both"
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NSISInstallerMode {
    /// Per-user installation (no admin rights required).
    ///
    /// Installs to `%LOCALAPPDATA%`.
    #[default]
    CurrentUser,

    /// Per-machine installation (requires admin rights).
    ///
    /// Installs to `%PROGRAMFILES%`.
    PerMachine,

    /// Let user choose during installation.
    Both,
}

/// NSIS compression algorithm.
///
/// Controls the compression method used for the NSIS installer executable.
///
/// # Comparison
///
/// | Algorithm | Speed | Size | Notes |
/// |-----------|-------|------|-------|
/// | None | Fastest | Largest | Development only |
/// | Zlib | Fast | Medium | Default, good balance |
/// | Bzip2 | Medium | Small | Better compression |
/// | LZMA | Slowest | Smallest | Best compression |
///
/// # Configuration
///
/// ```toml
/// [package.metadata.bundle.windows.nsis]
/// compression = "lzma"
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NsisCompression {
    /// No compression - fastest, largest size.
    None,

    /// zlib compression - good balance (default).
    #[default]
    Zlib,

    /// bzip2 compression - smaller than zlib.
    Bzip2,

    /// LZMA compression - smallest size, slowest.
    Lzma,
}

/// NSIS installer (.exe) configuration.
///
/// NSIS creates lightweight, customizable Windows installer executables.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.windows.nsis]
/// installer_mode = "perMachine"
/// compression = "lzma"
/// languages = ["en-US", "de-DE"]
/// ```
///
/// # See Also
///
/// - [`WindowsSettings`] - Windows installer configuration
/// - [`WixSettings`] - WiX MSI installer configuration
/// - [`NSISInstallerMode`] - Installation scope
/// - [`NsisCompression`] - Compression algorithms
#[derive(Clone, Debug, Default)]
pub struct NsisSettings {
    /// Path to custom NSIS template (.nsi file).
    ///
    /// Default: None (uses built-in template)
    pub template: Option<PathBuf>,

    /// Path to header image (150×57 pixels).
    ///
    /// Shown at top of installer window.
    ///
    /// Default: None
    pub header_image: Option<PathBuf>,

    /// Path to sidebar image (164×314 pixels).
    ///
    /// Shown on left side of installer window.
    ///
    /// Default: None
    pub sidebar_image: Option<PathBuf>,

    /// Path to installer icon (.ico file).
    ///
    /// Icon for the installer executable itself.
    ///
    /// Default: None (uses application icon)
    pub installer_icon: Option<PathBuf>,

    /// Installation mode (per-user, per-machine, or both).
    ///
    /// Default: [`NSISInstallerMode::CurrentUser`]
    pub install_mode: NSISInstallerMode,

    /// Supported installer languages.
    ///
    /// Example: `["en-US", "de-DE"]`
    ///
    /// Default: None (uses English)
    pub languages: Option<Vec<String>>,

    /// Compression algorithm for installer.
    ///
    /// Default: None (uses [`NsisCompression::Zlib`])
    pub compression: Option<NsisCompression>,
}
