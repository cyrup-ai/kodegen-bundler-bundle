//! Main bundler orchestration and coordination.
//!
//! This module provides the [`Bundler`] orchestrator that coordinates
//! platform-specific bundling operations to create native installers.

use crate::{
    bail,
    bundler::{BundledArtifact, PackageType, Result, Settings, error::ErrorExt},
};

use super::{checksum::calculate_sha256, tool_detection::HAS_MAKENSIS};

/// Main bundler orchestrator.
///
/// Coordinates the creation of platform-specific installers by delegating to
/// platform modules and collecting results.
///
/// # Platform Support
///
/// - **Linux**: Creates .deb, .rpm, and AppImage packages
/// - **macOS**: Creates .app bundles and .dmg disk images
/// - **Windows**: Creates .msi and .exe (NSIS) installers
///
/// # Examples
///
/// ```no_run
/// use kodegen_bundler_release::bundler::{Bundler, Settings, PackageType};
///
/// # async fn example(settings: Settings) -> kodegen_bundler_release::bundler::Result<()> {
/// // Create bundler
/// let bundler = Bundler::new(settings).await?;
///
/// // Bundle with platform defaults
/// let artifacts = bundler.bundle().await?;
///
/// // Or bundle specific types
/// let artifacts = bundler.bundle_types(&[
///     PackageType::Deb,
///     PackageType::AppImage,
/// ]).await?;
/// # Ok(())
/// # }
/// ```
pub struct Bundler {
    settings: Settings,
    #[cfg(target_os = "macos")]
    _temp_keychain: Option<kodegen_bundler_sign::macos::TempKeychain>,
}

impl std::fmt::Debug for Bundler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("Bundler");
        debug_struct.field("settings", &self.settings);
        #[cfg(target_os = "macos")]
        debug_struct.field(
            "_temp_keychain",
            &self._temp_keychain.as_ref().map(|_| "<TempKeychain>"),
        );
        debug_struct.finish()
    }
}

impl Bundler {
    /// Creates a new bundler with the given settings.
    ///
    /// # Arguments
    ///
    /// * `settings` - Bundler configuration from `SettingsBuilder`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kodegen_bundler_release::bundler::{Bundler, Settings};
    ///
    /// # async fn example(settings: Settings) -> kodegen_bundler_release::bundler::Result<()> {
    /// let bundler = Bundler::new(settings).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(settings: Settings) -> Result<Self> {
        #[cfg(target_os = "macos")]
        let _temp_keychain = super::signing::setup_macos_signing().await?;

        Ok(Self {
            settings,
            #[cfg(target_os = "macos")]
            _temp_keychain,
        })
    }

    /// Executes bundling operations for default platform types.
    ///
    /// Automatically determines which package types to create based on:
    /// 1. Explicit types from [`Settings::package_types()`] if set
    /// 2. Platform defaults otherwise (e.g., .deb + AppImage on Linux)
    ///
    /// # Returns
    ///
    /// Vector of [`BundledArtifact`] results, one per created package.
    ///
    /// # Platform Defaults
    ///
    /// - **Linux**: Deb, AppImage
    /// - **macOS**: MacOsBundle, Dmg
    /// - **Windows**: Nsis
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kodegen_bundler_release::bundler::Bundler;
    ///
    /// # async fn example(bundler: Bundler) -> kodegen_bundler_release::bundler::Result<()> {
    /// let artifacts = bundler.bundle().await?;
    /// println!("Created {} packages", artifacts.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bundle(&self) -> Result<Vec<BundledArtifact>> {
        let package_types = self.determine_platform_types();
        self.bundle_types(&package_types).await
    }

    /// Executes bundling operations for specific package types.
    ///
    /// Creates installers for the specified package types, regardless of platform
    /// defaults. Useful for creating only specific formats or cross-compiling.
    ///
    /// # Arguments
    ///
    /// * `types` - Slice of [`PackageType`] variants to create
    ///
    /// # Returns
    ///
    /// Vector of [`BundledArtifact`] results, one per created package.
    ///
    /// # Bundling Order
    ///
    /// Package types are created in the order provided, but some types have
    /// dependencies (e.g., DMG requires .app to exist first).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kodegen_bundler_release::bundler::{Bundler, PackageType};
    ///
    /// # async fn example(bundler: Bundler) -> kodegen_bundler_release::bundler::Result<()> {
    /// // Create only Debian and AppImage packages
    /// let artifacts = bundler.bundle_types(&[
    ///     PackageType::Deb,
    ///     PackageType::AppImage,
    /// ]).await?;
    ///
    /// for artifact in artifacts {
    ///     println!("Created: {}", artifact.package_type);
    ///     for path in &artifact.paths {
    ///         println!("  {}", path.display());
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Platform Compatibility
    ///
    /// Attempting to create a package type unsupported on the current platform
    /// will return an error.
    pub async fn bundle_types(&self, types: &[PackageType]) -> Result<Vec<BundledArtifact>> {
        let mut artifacts = Vec::new();

        for package_type in types {
            let paths = match package_type {
                #[cfg(target_os = "linux")]
                PackageType::Deb => {
                    crate::bundler::platform::linux::debian::bundle_project(&self.settings).await?
                }
                #[cfg(target_os = "linux")]
                PackageType::Rpm => {
                    crate::bundler::platform::linux::rpm::bundle_project(&self.settings).await?
                }
                #[cfg(target_os = "linux")]
                PackageType::AppImage => {
                    crate::bundler::platform::linux::appimage::bundle_project(&self.settings)
                        .await?
                }
                #[cfg(target_os = "macos")]
                PackageType::MacOsBundle => {
                    let identity = self._temp_keychain.as_ref().map(|k| k.signing_identity());
                    crate::bundler::platform::macos::app::bundle_project(&self.settings, identity)
                        .await?
                }
                #[cfg(target_os = "macos")]
                PackageType::Dmg => {
                    let identity = self._temp_keychain.as_ref().map(|k| k.signing_identity());
                    crate::bundler::platform::macos::dmg::bundle_project(&self.settings, identity)
                        .await?
                }
                #[cfg(target_os = "linux")]
                PackageType::Exe => {
                    crate::bundler::platform::windows::nsis::bundle_project(&self.settings).await?
                }
                #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                _ => {
                    bail!(
                        "Package type {:?} not supported on this platform",
                        package_type
                    );
                }
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                _ => {
                    bail!(
                        "Package type {:?} not supported on this platform",
                        package_type
                    );
                }
            };

            // Calculate artifact metadata
            let mut size = 0u64;
            for p in &paths {
                let metadata = tokio::fs::metadata(p)
                    .await
                    .fs_context("reading artifact metadata", p)?;
                size += metadata.len();
            }

            let checksum = if let Some(first_path) = paths.first() {
                calculate_sha256(first_path).await?
            } else {
                bail!(
                    "Platform bundler for {:?} returned no paths - this indicates a bundler bug",
                    package_type
                );
            };

            artifacts.push(BundledArtifact {
                package_type: *package_type,
                paths,
                size,
                checksum,
            });
        }

        Ok(artifacts)
    }

    /// Returns a reference to the bundler settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Determines which package types to build based on host platform.
    ///
    /// Returns explicit types from settings if specified, otherwise returns
    /// platform-appropriate defaults.
    fn determine_platform_types(&self) -> Vec<PackageType> {
        // If explicit types specified, use those
        if let Some(types) = self.settings.package_types() {
            return types.to_vec();
        }

        // Otherwise determine based on platform + available toolchains
        if cfg!(target_os = "linux") {
            let mut types = vec![
                PackageType::Deb,
                PackageType::Rpm, // Added (was missing)
                PackageType::AppImage,
            ];

            // Add Windows cross-compilation if makensis available
            if *HAS_MAKENSIS {
                log::debug!("makensis detected - enabling Windows NSIS cross-compilation");
                types.push(PackageType::Exe);
            } else {
                log::debug!("makensis not available - skipping NSIS installer");
            }

            types
        } else if cfg!(target_os = "macos") {
            vec![PackageType::MacOsBundle, PackageType::Dmg]
        } else {
            vec![]
        }
    }
}
