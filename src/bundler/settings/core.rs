//! Core Settings struct and implementations.

use super::{Arch, BundleBinary, BundleSettings, PackageSettings};
use std::path::{Path, PathBuf};

/// Main settings for bundler operations.
///
/// Central configuration for the bundler, constructed via [`SettingsBuilder`].
/// Contains package metadata, bundle settings, and platform-specific configuration.
///
/// # Examples
///
/// ```no_run
/// use kodegen_bundler_release::bundler::{Settings, SettingsBuilder, PackageSettings};
///
/// # fn example() -> kodegen_bundler_release::bundler::Result<()> {
/// let settings = SettingsBuilder::new()
///     .project_out_directory("target/release")
///     .package_settings(PackageSettings {
///         product_name: "MyApp".into(),
///         version: "1.0.0".into(),
///         description: "My application".into(),
///         ..Default::default()
///     })
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`SettingsBuilder`] - Builder for constructing Settings
/// - [`PackageSettings`] - Package metadata
/// - [`BundleSettings`] - Bundle configuration
#[derive(Clone, Debug)]
pub struct Settings {
    /// Package metadata.
    package: PackageSettings,

    /// Bundle configuration.
    bundle_settings: BundleSettings,

    /// Output directory for bundles.
    ///
    /// Typically `target/release` or `target/debug`.
    project_out_directory: PathBuf,

    /// Package types to create.
    ///
    /// None means use platform defaults (.deb on Debian, .rpm on Fedora, etc.).
    package_types: Option<Vec<crate::bundler::platform::PackageType>>,

    /// Binaries to bundle.
    binaries: Vec<BundleBinary>,

    /// Target triple (e.g., "x86_64-unknown-linux-gnu").
    ///
    /// Used for architecture detection.
    target: String,
}

impl Settings {
    /// Returns the product name.
    pub fn product_name(&self) -> &str {
        &self.package.product_name
    }

    /// Returns the version string.
    pub fn version_string(&self) -> &str {
        &self.package.version
    }

    /// Returns the package description.
    pub fn description(&self) -> &str {
        &self.package.description
    }

    /// Returns the project output directory.
    ///
    /// This is where compiled binaries are located.
    pub fn project_out_directory(&self) -> &Path {
        &self.project_out_directory
    }

    /// Detects the binary architecture from the target triple.
    ///
    /// Automatically determines the target architecture based on the Rust
    /// target triple (e.g., "x86_64-unknown-linux-gnu" → `Arch::X86_64`).
    pub fn binary_arch(&self) -> Arch {
        if self.target.starts_with("x86_64") {
            Arch::X86_64
        } else if self.target.starts_with('i') {
            Arch::X86
        } else if self.target.starts_with("aarch64") {
            Arch::AArch64
        } else if self.target.starts_with("arm") && self.target.ends_with("hf") {
            Arch::Armhf
        } else if self.target.starts_with("arm") {
            Arch::Armel
        } else if self.target.starts_with("riscv64") {
            Arch::Riscv64
        } else {
            Arch::X86_64 // fallback
        }
    }

    /// Returns the binaries to bundle.
    pub fn binaries(&self) -> &[BundleBinary] {
        &self.binaries
    }

    /// Returns the full path to a binary.
    ///
    /// Automatically appends `.exe` extension on Windows.
    pub fn binary_path(&self, binary: &BundleBinary) -> PathBuf {
        let mut path = self.project_out_directory.join(binary.name());

        if cfg!(target_os = "windows") {
            path.set_extension("exe");
        }

        path
    }

    /// Returns the bundle settings.
    pub fn bundle_settings(&self) -> &BundleSettings {
        &self.bundle_settings
    }

    /// Returns the package types to create.
    ///
    /// None means use platform defaults.
    pub fn package_types(&self) -> Option<&[crate::bundler::platform::PackageType]> {
        self.package_types.as_deref()
    }

    /// Loads and returns icon files with metadata.
    ///
    /// Reads icon files from paths specified in bundle settings and returns
    /// icon information including dimensions and format.
    ///
    /// # Errors
    ///
    /// Returns `IconPathError` if no icon paths are configured.
    pub fn icon_files(
        &self,
    ) -> crate::bundler::Result<Vec<crate::bundler::resources::icons::IconInfo>> {
        use crate::bundler::resources::icons::load_icons;

        if let Some(icon_paths) = &self.bundle_settings.icon {
            load_icons(icon_paths)
        } else {
            Err(crate::bundler::Error::IconPathError)
        }
    }

    /// Returns the package homepage URL.
    pub fn homepage(&self) -> Option<&str> {
        self.package.homepage.as_deref()
    }

    /// Returns the package authors.
    pub fn authors(&self) -> Option<&[String]> {
        self.package.authors.as_deref()
    }

    /// Creates a new Settings instance (used by SettingsBuilder).
    pub(super) fn new(
        package: PackageSettings,
        bundle_settings: BundleSettings,
        project_out_directory: PathBuf,
        package_types: Option<Vec<crate::bundler::platform::PackageType>>,
        binaries: Vec<BundleBinary>,
        target: String,
    ) -> Self {
        Self {
            package,
            bundle_settings,
            project_out_directory,
            package_types,
            binaries,
            target,
        }
    }
}
