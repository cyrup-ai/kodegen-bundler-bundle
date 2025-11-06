//! NSIS utility functions.
//!
//! Helper functions for architecture mapping, version formatting,
//! compression settings, and file operations.

use crate::bundler::{
    error::{Error, ErrorExt, Result},
    settings::{Arch, NSISInstallerMode, NsisCompression},
};
use std::path::Path;
use tokio::io::AsyncWriteExt;

/// Map architecture to NSIS arch string.
///
/// Converts bundler architecture enum to NSIS-compatible architecture identifier.
pub fn map_arch(arch: Arch) -> Result<&'static str> {
    match arch {
        Arch::X86_64 => Ok("x64"),
        Arch::X86 => Ok("x86"),
        Arch::AArch64 => Ok("arm64"),
        _ => Err(Error::ArchError(format!(
            "Unsupported architecture for NSIS: {:?}",
            arch
        ))),
    }
}

/// Map compression setting to NSIS compression string.
///
/// Returns the compression algorithm name for use in NSIS scripts.
/// Defaults to LZMA if no compression is specified.
pub fn map_compression(compression: Option<NsisCompression>) -> &'static str {
    match compression.unwrap_or(NsisCompression::Lzma) {
        NsisCompression::None => "none",
        NsisCompression::Zlib => "zlib",
        NsisCompression::Bzip2 => "bzip2",
        NsisCompression::Lzma => "lzma",
    }
}

/// Map install mode to NSIS mode string.
///
/// Converts bundler install mode enum to NSIS-compatible mode identifier.
pub fn map_install_mode(mode: NSISInstallerMode) -> &'static str {
    match mode {
        NSISInstallerMode::CurrentUser => "currentUser",
        NSISInstallerMode::PerMachine => "perMachine",
        NSISInstallerMode::Both => "both",
    }
}

/// Format version string for NSIS VIProductVersion.
///
/// NSIS requires exactly 4 numeric parts (major.minor.patch.build).
/// This function normalizes version strings to meet that requirement:
/// - "1" -> "1.0.0.0"
/// - "1.2" -> "1.2.0.0"
/// - "1.2.3" -> "1.2.3.0"
/// - "1.2.3.4" -> "1.2.3.4"
/// - "1.2.3.4.5" -> "1.2.3.4" (truncates to first 4)
pub fn format_version_for_nsis(version: &str) -> Result<String> {
    // Parse version components
    let parts: Vec<&str> = version.split('.').collect();

    match parts.len() {
        1 => Ok(format!("{}.0.0.0", parts[0])),
        2 => Ok(format!("{}.{}.0.0", parts[0], parts[1])),
        3 => Ok(format!("{}.{}.{}.0", parts[0], parts[1], parts[2])),
        4 => Ok(version.to_string()),
        _ => {
            // More than 4 parts, take first 4
            Ok(format!(
                "{}.{}.{}.{}",
                parts[0], parts[1], parts[2], parts[3]
            ))
        }
    }
}

/// Write file with UTF-8 BOM (required by NSIS).
///
/// NSIS requires installer scripts to be encoded with UTF-8 BOM (byte order mark).
/// This function writes the BOM (EF BB BF) followed by the content.
pub async fn write_utf8_bom(path: &Path, content: &str) -> Result<()> {
    let mut file = tokio::fs::File::create(path)
        .await
        .fs_context("creating NSI script file", path)?;

    // Write UTF-8 BOM: EF BB BF
    file.write_all(&[0xEF, 0xBB, 0xBF])
        .await
        .fs_context("writing UTF-8 BOM", path)?;
    file.write_all(content.as_bytes())
        .await
        .fs_context("writing NSI content", path)?;
    file.flush().await.fs_context("flushing NSI file", path)?;

    Ok(())
}
