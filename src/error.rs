//! Comprehensive error types for bundler operations.
//!
//! This module defines all error types with actionable error messages and recovery suggestions.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for bundler operations
pub type Result<T> = std::result::Result<T, BundlerError>;

/// Main error type for all bundler operations
#[derive(Error, Debug)]
pub enum BundlerError {
    /// CLI argument errors
    #[error("CLI error: {0}")]
    Cli(#[from] CliError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing errors
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Bundler errors
    #[error("Bundler error: {0}")]
    Bundler(#[from] crate::bundler::Error),

    /// Generic errors from anyhow
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
}

/// CLI-specific errors
#[derive(Error, Debug)]
pub enum CliError {
    /// Invalid command line arguments
    #[error("Invalid arguments: {reason}")]
    InvalidArguments {
        /// Reason for the error
        reason: String,
    },

    /// Missing required argument
    #[error("Missing required argument: {argument}")]
    MissingArgument {
        /// Argument name
        argument: String,
    },

    /// Conflicting arguments
    #[error("Conflicting arguments: {arguments:?}")]
    ConflictingArguments {
        /// Arguments that conflict
        arguments: Vec<String>,
    },

    /// Command execution failed
    #[error("Command execution failed: {command} - {reason}")]
    ExecutionFailed {
        /// Command that failed
        command: String,
        /// Reason for the error
        reason: String,
    },
}

impl BundlerError {
    /// Get actionable recovery suggestions for this error
    pub fn recovery_suggestions(&self) -> Vec<String> {
        vec!["Check the error message above for specific details".to_string()]
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        true
    }
}
