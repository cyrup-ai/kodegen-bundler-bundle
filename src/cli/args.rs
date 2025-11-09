//! Command line argument parsing and validation.
//!
//! This module provides comprehensive CLI argument parsing using clap,
//! with proper validation and error handling.

use clap::Parser;
use std::path::PathBuf;

/// Platform package bundler for Rust binaries
#[derive(Parser, Debug)]
#[command(
    name = "kodegen_bundler_bundle",
    version,
    about = "Platform package bundler for Rust binaries",
    long_about = "Creates platform-specific packages (.deb, .rpm, .dmg, AppImage, .exe) for Rust binaries.

Clones repository from GitHub to tmp, builds binary, creates package, moves to output path.

Usage:
  kodegen_bundler_bundle --source . --platform deb --output-binary /tmp/myapp.deb
  kodegen_bundler_bundle --source cyrup-ai/kodegen --platform dmg --output-binary ./kodegen.dmg
  kodegen_bundler_bundle --source https://github.com/user/repo --platform nsis --output-binary setup.exe

Exit code 0 = artifact guaranteed to exist at output path."
)]
pub struct Args {
    /// Source repository (local path, GitHub org/repo, or GitHub URL)
    #[arg(short = 's', long, value_name = "SOURCE")]
    pub source: String,

    /// Platform to bundle: deb, rpm, dmg, macos-bundle, nsis, appimage
    #[arg(short, long, value_name = "PLATFORM")]
    pub platform: String,

    /// Output path for the created artifact
    ///
    /// The bundler will move the created artifact to this exact path.
    /// The bundler will create parent directories if they don't exist.
    /// The filename should include the architecture (e.g., kodegen_0.1.0_arm64.deb).
    ///
    /// Contract: Exit code 0 guarantees the artifact exists at this path.
    #[arg(short = 'o', long, value_name = "PATH")]
    pub output_binary: PathBuf,
}

impl Args {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Validate arguments for consistency
    pub fn validate(&self) -> Result<(), String> {
        // Validate source format (basic validation - full validation happens during resolve)
        if self.source.is_empty() {
            return Err("Source cannot be empty".to_string());
        }

        // Validate platform
        let valid_platforms = ["deb", "rpm", "dmg", "macos-bundle", "nsis", "appimage"];
        if !valid_platforms.contains(&self.platform.as_str()) {
            return Err(format!(
                "Invalid platform: {}. Valid platforms: {}",
                self.platform,
                valid_platforms.join(", ")
            ));
        }

        Ok(())
    }
}

/// Configuration derived from command line arguments
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Output manager for colored terminal output
    output: super::OutputManager,
}

impl From<&Args> for RuntimeConfig {
    fn from(_args: &Args) -> Self {
        let output = super::OutputManager::new(
            true,  // Always verbose
            false, // Never quiet
        );

        Self { output }
    }
}

impl RuntimeConfig {
    /// Get a reference to the output manager
    pub fn output(&self) -> &super::OutputManager {
        &self.output
    }

    /// Print verbose message if in verbose mode
    pub fn verbose_println(&self, message: &str) -> std::io::Result<()> {
        self.output.verbose(message)
    }

    /// Print warning message if not in quiet mode
    pub fn warning_println(&self, message: &str) -> std::io::Result<()> {
        self.output.warn(message)
    }

    /// Print success message if not in quiet mode
    pub fn success_println(&self, message: &str) -> std::io::Result<()> {
        self.output.success(message)
    }

    /// Print success message (alias for success_println for convenience)
    pub fn success(&self, message: &str) -> std::io::Result<()> {
        self.output.success(message)
    }

    /// Print warning message (alias for warning_println for convenience)
    pub fn warn(&self, message: &str) -> std::io::Result<()> {
        self.output.warn(message)
    }

    /// Print progress message
    pub fn progress(&self, message: &str) -> std::io::Result<()> {
        self.output.progress(message)
    }

    /// Print section header
    pub fn section(&self, title: &str) -> std::io::Result<()> {
        self.output.section(title)
    }

    /// Print indented text
    pub fn indent(&self, message: &str) -> std::io::Result<()> {
        self.output.indent(message)
    }
}
