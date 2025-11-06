//! Docker container integration for cross-platform bundling.
//!
//! This module enables the `bundle` command to automatically use Docker containers
//! when building packages for platforms other than the host OS.
//!
//! # Example
//!
//! On macOS, running `bundle --all-platforms` will:
//! - Build macOS packages (.app, .dmg) natively
//! - Build Linux/Windows packages (.deb, .rpm, AppImage, .msi, .exe) in a Linux container with Wine
//!
//! # Architecture
//!
//! The Linux container (defined in `.devcontainer/Dockerfile`) includes:
//! - Rust toolchain (nightly matching rust-toolchain.toml)
//! - Wine + .NET 4.0 (for running WiX to create .msi installers)
//! - NSIS (for creating .exe installers)
//! - RPM/DEB tools (for creating Linux packages)
//! - linuxdeploy (for creating AppImages)
//!
//! # Module Structure
//!
//! - `artifacts` - Artifact verification and discovery
//! - `artifact_manager` - Artifact discovery, validation, and file management
//! - `bundler` - Main container bundler implementation
//! - `container_runner` - Docker container execution and process streaming
//! - `guard` - RAII guard for container cleanup
//! - `image` - Docker image management and building
//! - `limits` - Resource limits for containers
//! - `oom_detector` - Out-of-memory detection and error reporting
//! - `platform` - Platform detection and classification

mod artifact_manager;
mod artifacts;
pub mod bundler;
mod container_runner;
mod guard;
mod image;
pub mod limits;
mod oom_detector;
mod platform;

// Re-export public API
pub use bundler::ContainerBundler;
pub use limits::ContainerLimits;
