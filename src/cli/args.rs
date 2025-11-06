//! Command line argument parsing and validation.
#![allow(dead_code)] // Public API - items may be used by external consumers

//!
//! This module provides comprehensive CLI argument parsing using clap,
//! with proper validation and error handling.

use super::retry_config::RetryConfig;
use clap::Parser;
use std::path::PathBuf;

/// Platform package bundler for Rust binaries
#[derive(Parser, Debug)]
#[command(
    name = "kodegen_bundler_bundle",
    version,
    disable_version_flag = true,
    about = "Platform package bundler for Rust binaries",
    long_about = "Creates platform-specific packages (.deb, .rpm, .dmg, .msi, AppImage) for Rust binaries.

Usage:
  kodegen_bundler_bundle --repo-path /path/to/repo --platform deb --binary-name myapp --version 1.0.0
  kodegen_bundler_bundle -r . -p dmg -b kodegen -v 0.1.3 --target x86_64-apple-darwin
  kodegen_bundler_bundle --repo-path /workspace --platform rpm --binary-name tool --version 2.1.0 --no-build"
)]
pub struct Args {
    /// Path to repository root
    #[arg(short = 'r', long, value_name = "PATH")]
    pub repo_path: PathBuf,

    /// Platform to bundle: deb, rpm, dmg, macos-bundle, nsis, appimage
    #[arg(short, long, value_name = "PLATFORM")]
    pub platform: String,

    /// Binary name to bundle
    #[arg(short, long, value_name = "NAME")]
    pub binary_name: String,

    /// Package version
    #[arg(short, long, value_name = "VERSION")]
    pub version: String,

    /// Output directory for artifacts
    #[arg(long, value_name = "PATH")]
    pub output_dir: Option<PathBuf>,

    /// Target architecture (e.g., x86_64-apple-darwin)
    #[arg(short, long)]
    pub target: Option<String>,

    /// Skip building binary (assume already built)
    #[arg(long)]
    pub no_build: bool,

    /// Enable verbose output
    #[arg(short = 'V', long)]
    pub verbose: bool,

    /// Optional output path for the created artifact
    ///
    /// If specified, the bundler will move the created artifact to this exact path.
    /// The bundler will create parent directories if they don't exist.
    /// The filename should include the architecture (e.g., kodegen_0.1.0_arm64.deb).
    ///
    /// Contract: Exit code 0 guarantees the artifact exists at this path.
    #[arg(short = 'o', long, value_name = "PATH")]
    pub output_binary: Option<PathBuf>,

    // ===== DOCKER CONTAINER LIMITS =====
    /// Docker container memory limit (e.g., "2g", "4096m")
    ///
    /// Defaults to auto-detected safe limit (50% of host RAM, min 2GB, max 16GB)
    #[arg(long, env = "KODEGEN_DOCKER_MEMORY")]
    pub docker_memory: Option<String>,

    /// Docker container memory + swap limit (e.g., "6g", "8192m")
    ///
    /// Must be ≥ memory limit. Defaults to memory + 2GB if not specified.
    #[arg(long, env = "KODEGEN_DOCKER_MEMORY_SWAP")]
    pub docker_memory_swap: Option<String>,

    /// Docker container CPU limit (e.g., "2.0", "4", "1.5")
    ///
    /// Supports fractional values. Defaults to auto-detected (50% of host cores, min 2)
    #[arg(long, env = "KODEGEN_DOCKER_CPUS")]
    pub docker_cpus: Option<String>,

    /// Docker container process limit
    ///
    /// Maximum number of processes. Defaults to 1000.
    #[arg(long, env = "KODEGEN_DOCKER_PIDS_LIMIT")]
    pub docker_pids_limit: Option<u32>,
}

impl Args {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Check if running in verbose mode
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Validate arguments for consistency
    pub fn validate(&self) -> Result<(), String> {
        // Validate repo path
        if !self.repo_path.exists() {
            return Err(format!(
                "Repository path does not exist: {}",
                self.repo_path.display()
            ));
        }
        if !self.repo_path.is_dir() {
            return Err(format!(
                "Repository path is not a directory: {}",
                self.repo_path.display()
            ));
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

        // Validate binary name is not empty
        if self.binary_name.is_empty() {
            return Err("Binary name cannot be empty".to_string());
        }

        // Validate version is not empty
        if self.version.is_empty() {
            return Err("Version cannot be empty".to_string());
        }

        // Validate output directory if provided
        if let Some(ref output_dir) = self.output_dir
            && let Some(parent) = output_dir.parent()
            && !parent.exists()
        {
            return Err(format!(
                "Output directory parent does not exist: {}",
                parent.display()
            ));
        }

        Ok(())
    }
}

/// Configuration derived from command line arguments
#[derive(Debug)]
pub struct RuntimeConfig {
    /// Repository root path
    pub repo_path: PathBuf,
    /// Verbosity level
    pub verbosity: VerbosityLevel,
    /// Output manager for colored terminal output
    output: super::OutputManager,
    /// Docker container memory limit
    pub docker_memory: Option<String>,
    /// Docker container memory + swap limit  
    pub docker_memory_swap: Option<String>,
    /// Docker container CPU limit
    pub docker_cpus: Option<String>,
    /// Docker container process limit
    pub docker_pids_limit: Option<u32>,
    /// Retry configuration for various operations
    pub retry_config: RetryConfig,
}

/// Verbosity level for output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbosityLevel {
    /// Minimal output
    Quiet,
    /// Standard output
    Normal,
    /// Detailed output
    Verbose,
}

impl From<&Args> for RuntimeConfig {
    fn from(args: &Args) -> Self {
        let verbosity = if args.verbose {
            VerbosityLevel::Verbose
        } else {
            VerbosityLevel::Normal
        };

        let output = super::OutputManager::new(
            verbosity == VerbosityLevel::Verbose,
            false, // Never quiet for bundler
        );

        Self {
            repo_path: args.repo_path.clone(),
            verbosity,
            output,
            // Docker container limits
            docker_memory: args.docker_memory.clone(),
            docker_memory_swap: args.docker_memory_swap.clone(),
            docker_cpus: args.docker_cpus.clone(),
            docker_pids_limit: args.docker_pids_limit,
            retry_config: RetryConfig::from_env(),
        }
    }
}

impl RuntimeConfig {
    /// Check if output should be suppressed
    pub fn is_quiet(&self) -> bool {
        self.verbosity == VerbosityLevel::Quiet
    }

    /// Check if verbose output is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbosity == VerbosityLevel::Verbose
    }

    /// Print message if not in quiet mode
    pub fn println(&self, message: &str) {
        self.output.println(message);
    }

    /// Print verbose message if in verbose mode
    pub fn verbose_println(&self, message: &str) {
        self.output.verbose(message);
    }

    /// Print error message (always shown)
    pub fn error_println(&self, message: &str) {
        self.output.error(message);
    }

    /// Print warning message if not in quiet mode
    pub fn warning_println(&self, message: &str) {
        self.output.warn(message);
    }

    /// Print success message if not in quiet mode
    pub fn success_println(&self, message: &str) {
        self.output.success(message);
    }

    /// Print success message (alias for success_println for convenience)
    pub fn success(&self, message: &str) {
        self.output.success(message);
    }

    /// Print warning message (alias for warning_println for convenience)
    pub fn warn(&self, message: &str) {
        self.output.warn(message);
    }

    /// Print error message (always shown, alias for error_println)
    pub fn error(&self, message: &str) {
        self.output.error(message);
    }

    /// Print info message
    pub fn info(&self, message: &str) {
        self.output.info(message);
    }

    /// Print progress message
    pub fn progress(&self, message: &str) {
        self.output.progress(message);
    }

    /// Print section header
    pub fn section(&self, title: &str) {
        self.output.section(title);
    }

    /// Print indented text
    pub fn indent(&self, message: &str) {
        self.output.indent(message);
    }
}
