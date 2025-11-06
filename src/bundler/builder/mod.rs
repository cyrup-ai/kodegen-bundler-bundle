//! Bundle orchestration and coordination.
#![allow(dead_code)] // Public API - items may be used by external consumers

//!
//! This module provides the main [`Bundler`] orchestrator that coordinates
//! platform-specific bundling operations to create native installers.
//!
//! # Overview
//!
//! The bundler:
//! 1. Reads configuration from [`Settings`]
//! 2. Determines which package types to create
//! 3. Delegates to platform-specific modules
//! 4. Calculates checksums and metadata
//! 5. Returns [`BundledArtifact`] results
//!
//! # Example
//!
//! ```no_run
//! use kodegen_bundler_release::bundler::{Bundler, SettingsBuilder, PackageSettings};
//!
//! # async fn example() -> kodegen_bundler_release::bundler::Result<()> {
//! let settings = SettingsBuilder::new()
//!     .project_out_directory("target/release")
//!     .package_settings(PackageSettings {
//!         product_name: "MyApp".into(),
//!         version: "1.0.0".into(),
//!         description: "My application".into(),
//!         ..Default::default()
//!     })
//!     .build()?;
//!
//! let bundler = Bundler::new(settings).await?;
//! let artifacts = bundler.bundle().await?;
//!
//! for artifact in artifacts {
//!     println!("Created: {:?} ({} bytes)", artifact.package_type, artifact.size);
//!     println!("SHA256: {}", artifact.checksum);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Module Organization
//!
//! This module is organized into the following submodules:
//!
//! - [`checksum`] - SHA256 checksum calculation for artifacts
//! - [`orchestrator`] - Main [`Bundler`] struct and bundling operations
//! - [`signing`] - Code signing setup (macOS keychain management)
//! - [`tool_detection`] - External tool availability checking

mod checksum;
mod orchestrator;
mod signing;
mod tool_detection;

// Re-export the main Bundler type for backwards compatibility
pub use orchestrator::Bundler;
