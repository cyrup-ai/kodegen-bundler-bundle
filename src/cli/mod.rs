//! Command line interface for kodegen bundler.
//!
//! This module provides a comprehensive CLI for bundler operations,
//! with proper argument parsing, command execution, and user feedback.

mod args;
pub mod commands;
mod docker;
mod output;
mod retry_config;

pub use args::{Args, RuntimeConfig};
pub use output::OutputManager;

use crate::error::Result;

/// Main CLI entry point
pub async fn run() -> Result<i32> {
    let _args = Args::parse_args();
    // TODO: Implement bundler command execution
    Ok(0)
}

/// Parse arguments without executing (for testing)
#[allow(dead_code)] // Public API - preserved for external consumers
pub fn parse_args() -> Args {
    Args::parse_args()
}

/// Validate arguments without executing (for testing)
#[allow(dead_code)] // Public API - preserved for external consumers
pub fn validate_args(args: &Args) -> std::result::Result<(), String> {
    args.validate()
}

/// Create runtime configuration from arguments
#[allow(dead_code)] // Public API - preserved for external consumers
pub fn create_runtime_config(args: &Args) -> RuntimeConfig {
    RuntimeConfig::from(args)
}
