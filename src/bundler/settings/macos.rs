//! macOS platform-specific settings.

use std::collections::HashMap;
use std::path::PathBuf;

/// macOS application bundle (.app) configuration.
///
/// Configures the creation of macOS `.app` bundles with optional code signing
/// and notarization.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.macos]
/// minimum_system_version = "10.15"
/// signing_identity = "Developer ID Application: Your Name (TEAMID)"
/// entitlements = "entitlements.plist"
/// ```
///
/// # Code Signing
///
/// See [`kodegen_sign`](../../sign/index.html) for automated certificate provisioning
/// and code signing setup.
///
/// # See Also
///
/// - [`DmgSettings`] - DMG disk image configuration
/// - [`WindowsSettings`] - Windows installer configuration
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MacOsSettings {
    /// System frameworks to bundle with the application.
    ///
    /// Example: `["WebKit.framework", "Security.framework"]`
    ///
    /// Default: None (no additional frameworks)
    #[serde(default)]
    pub frameworks: Option<Vec<String>>,

    /// Minimum macOS version required (LSMinimumSystemVersion).
    ///
    /// Example: "10.15", "11.0", "12.0"
    ///
    /// Default: None (uses current SDK version)
    #[serde(default)]
    pub minimum_system_version: Option<String>,

    /// Code signing identity name.
    ///
    /// Example: "Developer ID Application: Your Name (TEAMID)"
    ///
    /// Use "-" for ad-hoc signing (development only).
    ///
    /// Default: None (unsigned)
    #[serde(default)]
    pub signing_identity: Option<String>,

    /// Path to entitlements.plist for code signing.
    ///
    /// Required for certain macOS features (network, camera, etc.).
    ///
    /// Default: None
    #[serde(default)]
    pub entitlements: Option<PathBuf>,

    /// Custom files to include (destination -> source).
    ///
    /// Default: Empty
    #[serde(default)]
    pub files: HashMap<PathBuf, PathBuf>,

    /// Skip notarization with Apple.
    ///
    /// Notarization is required for distribution outside the Mac App Store.
    /// Only skip for development/testing.
    ///
    /// Default: false (notarization enabled)
    #[serde(default)]
    pub skip_notarization: bool,

    /// Skip stapling the notarization ticket.
    ///
    /// Stapling attaches the notarization ticket to the bundle for offline verification.
    ///
    /// Default: false (stapling enabled)
    #[serde(default)]
    pub skip_stapling: bool,
}

/// macOS DMG disk image configuration.
///
/// Configures the appearance and layout of macOS disk image installers.
///
/// # Configuration
///
/// Add to `Cargo.toml`:
///
/// ```toml
/// [package.metadata.bundle.dmg]
/// background = "assets/dmg-background.png"
/// window_size = [540, 380]
/// ```
///
/// # See Also
///
/// - [`MacOsSettings`] - macOS app bundle configuration
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct DmgSettings {
    /// Path to background image for DMG window.
    ///
    /// Should be PNG format. Recommended size: 540x380 pixels.
    ///
    /// Default: None (plain background)
    #[serde(default)]
    pub background: Option<PathBuf>,

    /// DMG window size (width, height) in pixels.
    ///
    /// Default: None (uses default size)
    #[serde(default)]
    pub window_size: Option<(u32, u32)>,
}
