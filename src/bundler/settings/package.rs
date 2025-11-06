//! Package metadata and configuration.

/// Package metadata and configuration.
///
/// Contains core package information used across all bundling platforms.
/// This typically maps from `Cargo.toml` `[package]` section.
///
/// # Examples
///
/// ```no_run
/// use kodegen_bundler_release::bundler::PackageSettings;
///
/// let settings = PackageSettings {
///     product_name: "MyApp".into(),
///     version: "1.0.0".into(),
///     description: "An awesome application".into(),
///     homepage: Some("https://example.com".into()),
///     authors: Some(vec!["Author Name <email@example.com>".into()]),
///     default_run: Some("myapp".into()),
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct PackageSettings {
    /// Product name displayed to users.
    ///
    /// This is the human-readable name shown in installers and system menus.
    /// Usually derived from `Cargo.toml` `package.name`.
    pub product_name: String,

    /// Version string in semantic versioning format.
    ///
    /// Example: "1.0.0", "0.2.3-beta.1"
    pub version: String,

    /// Brief description of the application.
    ///
    /// Used in package managers and installer descriptions.
    pub description: String,

    /// Homepage URL for the application.
    ///
    /// Default: None
    pub homepage: Option<String>,

    /// List of package authors.
    ///
    /// Format: "Name <email@example.com>"
    ///
    /// Default: None
    pub authors: Option<Vec<String>>,

    /// Default binary to run when multiple binaries exist.
    ///
    /// If the package contains multiple binaries, this specifies which one
    /// should be the primary executable.
    ///
    /// Default: None (uses first binary)
    pub default_run: Option<String>,
}
