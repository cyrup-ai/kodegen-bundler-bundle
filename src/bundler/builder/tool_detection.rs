//! External tool detection and availability checking.
//!
//! This module provides compile-time and runtime detection of external tools
//! required for various bundling operations (e.g., makensis for Windows NSIS installers).

use std::sync::LazyLock;

/// Check if makensis is available for NSIS installer creation.
///
/// Cached result to avoid repeated subprocess calls during bundling.
pub static HAS_MAKENSIS: LazyLock<bool> = LazyLock::new(|| match which::which("makensis") {
    Ok(path) => {
        log::debug!("Found makensis at: {}", path.display());

        match std::process::Command::new(&path).arg("-VERSION").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                log::info!("✓ makensis available: {}", version.trim());
                true
            }
            Ok(output) => {
                log::warn!(
                    "makensis found at {} but -VERSION check failed (exit code: {:?}). \
                         NSIS installers will be skipped. \
                         Stderr: {}",
                    path.display(),
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            }
            Err(e) => {
                log::warn!(
                    "makensis found at {} but failed to execute: {}. \
                         NSIS installers will be skipped. \
                         Check file permissions.",
                    path.display(),
                    e
                );
                false
            }
        }
    }
    Err(e) => {
        log::debug!(
            "makensis not found in PATH: {}. NSIS installers will be skipped.",
            e
        );
        false
    }
});
