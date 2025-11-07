//! Multi-platform bundler library for creating native installers
//!
//! This library provides the core bundling functionality for creating:
//! - Linux packages (.deb, .rpm, AppImage)
//! - macOS packages (.dmg, .app bundles)
//! - Windows installers (.exe via NSIS)
//!
//! It can be used both as a CLI tool and as a library dependency.

pub mod bundler;
pub mod cli;
pub mod error;
pub mod metadata;

// Re-export commonly used types
pub use error::{BundlerError, CliError, Result};
