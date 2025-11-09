//! Configuration structures for bundling operations.
//!
//! This module provides comprehensive configuration types for multi-platform
//! bundling, including package metadata, platform-specific settings, and
//! builder patterns for constructing settings.

#![allow(dead_code)] // Public API - Settings structs preserve all fields for external consumers

mod arch;
mod builder;
mod bundle;
mod core;
mod linux;
mod macos;
mod package;
mod windows;

// Re-export all public types
pub use arch::Arch;
pub use builder::SettingsBuilder;
pub use bundle::{BundleBinary, BundleSettings, CategorySettings};
pub use core::Settings;
pub use linux::{AppImageSettings, DebianSettings, RpmSettings};
pub use macos::{DmgSettings, MacOsSettings};
pub use package::PackageSettings;
pub use windows::WindowsSettings;
