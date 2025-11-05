//! Command execution functions for bundler operations.
//!
//! This module provides devcontainer management for Docker-based builds.

// Submodules
mod devcontainer;

pub use devcontainer::copy_embedded_devcontainer;
