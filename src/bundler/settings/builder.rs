//! Builder for constructing Settings.

use super::{BundleBinary, BundleSettings, PackageSettings, Settings};
use std::path::{Path, PathBuf};

/// Builder for constructing [`Settings`].
///
/// Provides a fluent API for building bundler settings with validation.
///
/// # Examples
///
/// ```no_run
/// use kodegen_bundler_release::bundler::{SettingsBuilder, PackageSettings, BundleBinary};
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
///     .binaries(vec![
///         BundleBinary::new("myapp".into(), true),
///     ])
///     .target("x86_64-unknown-linux-gnu".into())
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`Settings`] - The built settings struct
#[derive(Default)]
pub struct SettingsBuilder {
    project_out_directory: Option<PathBuf>,
    package_settings: Option<PackageSettings>,
    bundle_settings: BundleSettings,
    package_types: Option<Vec<crate::bundler::platform::PackageType>>,
    binaries: Vec<BundleBinary>,
    target: Option<String>,
}

impl SettingsBuilder {
    /// Creates a new settings builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the project output directory.
    ///
    /// This should point to where compiled binaries are located,
    /// typically `target/release` or `target/debug`.
    ///
    /// # Required
    ///
    /// This field is required for building.
    pub fn project_out_directory<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.project_out_directory = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets package metadata.
    ///
    /// # Required
    ///
    /// This field is required for building.
    pub fn package_settings(mut self, settings: PackageSettings) -> Self {
        self.package_settings = Some(settings);
        self
    }

    /// Sets bundle configuration.
    ///
    /// Default: Empty [`BundleSettings`]
    pub fn bundle_settings(mut self, settings: BundleSettings) -> Self {
        self.bundle_settings = settings;
        self
    }

    /// Sets specific package types to create.
    ///
    /// If not set, uses platform defaults (e.g., .deb on Debian systems).
    ///
    /// Default: None (platform defaults)
    pub fn package_types(mut self, types: Vec<crate::bundler::platform::PackageType>) -> Self {
        self.package_types = Some(types);
        self
    }

    /// Sets binaries to bundle.
    ///
    /// Default: Empty (no binaries bundled)
    pub fn binaries(mut self, binaries: Vec<BundleBinary>) -> Self {
        self.binaries = binaries;
        self
    }

    /// Sets target triple.
    ///
    /// If not set, uses the `TARGET` environment variable or current architecture.
    ///
    /// Default: Current architecture
    pub fn target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }

    /// Builds the settings.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing:
    /// - `project_out_directory`
    /// - `package_settings`
    pub fn build(self) -> crate::bundler::Result<Settings> {
        use crate::bundler::error::Context;

        let target = self.target.unwrap_or_else(|| {
            std::env::var("TARGET").unwrap_or_else(|_| std::env::consts::ARCH.to_string())
        });

        Ok(Settings::new(
            self.package_settings
                .context("package_settings is required")?,
            self.bundle_settings,
            self.project_out_directory
                .context("project_out_directory is required")?,
            self.package_types,
            self.binaries,
            target,
        ))
    }
}
