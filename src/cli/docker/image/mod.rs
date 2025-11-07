//! Docker image management for release builds.
#![allow(dead_code)] // Public API - items may be used by external consumers

//!
//! Handles building and maintaining the builder Docker image used for
//! cross-platform package creation.

mod availability;
mod builder;
mod config;
mod manager;
mod staleness;
mod utils;

// Re-export public API
pub use config::BUILDER_IMAGE_NAME;
pub use manager::ensure_image_built;
