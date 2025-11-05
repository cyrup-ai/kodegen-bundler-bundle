//! Command line interface for kodegen bundler.
//!
//! This module provides a comprehensive CLI for bundler operations,
//! with proper argument parsing, command execution, and user feedback.

mod args;
pub mod commands;
mod docker;
mod output;
mod retry_config;

pub use args::{Args, RuntimeConfig, VerbosityLevel};
pub use output::OutputManager;
pub use retry_config::RetryConfig;

use crate::error::Result;

/// Main CLI entry point
pub async fn run() -> Result<i32> {
    let _args = Args::parse_args();
    // TODO: Implement bundler command execution
    Ok(0)
}

/// Parse arguments without executing (for testing)
pub fn parse_args() -> Args {
    Args::parse_args()
}

/// Validate arguments without executing (for testing)
pub fn validate_args(args: &Args) -> std::result::Result<(), String> {
    args.validate()
}

/// Create runtime configuration from arguments
pub fn create_runtime_config(args: &Args) -> RuntimeConfig {
    RuntimeConfig::from(args)
}
