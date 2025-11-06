//! Artifact checksum calculation.
//!
//! This module provides SHA256 checksum calculation for bundled artifacts,
//! supporting both single files and directory trees (e.g., macOS .app bundles).

use crate::{bail, bundler::Result, bundler::error::ErrorExt};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

/// Calculates SHA256 checksum of a file or directory.
///
/// For files: Reads in 8KB chunks and computes the SHA-256 hash.
/// For directories: Recursively hashes all files in deterministic order.
///
/// # Arguments
///
/// * `path` - Path to file or directory to hash
///
/// # Returns
///
/// * `Ok(String)` - Hex-encoded SHA-256 hash (64 characters)
/// * `Err` - If path cannot be read or is neither file nor directory
pub async fn calculate_sha256(path: &std::path::Path) -> Result<String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(crate::bundler::Error::IoError)?;

    if metadata.is_file() {
        // Hash a single file
        calculate_file_sha256(path).await
    } else if metadata.is_dir() {
        // Hash directory tree (e.g., macOS .app bundles)
        calculate_directory_sha256(path).await
    } else {
        bail!("Path is neither file nor directory: {}", path.display())
    }
}

/// Calculates SHA256 checksum of a single file.
///
/// Reads the file in 8KB chunks to handle large files efficiently.
///
/// # Arguments
///
/// * `file_path` - Path to file to hash
///
/// # Returns
///
/// * `Ok(String)` - Hex-encoded SHA-256 hash
/// * `Err` - If file cannot be read
async fn calculate_file_sha256(file_path: &std::path::Path) -> Result<String> {
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(crate::bundler::Error::IoError)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        let n = file
            .read(&mut buffer)
            .await
            .map_err(crate::bundler::Error::IoError)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Calculates SHA256 checksum of a directory tree.
///
/// Recursively traverses the directory, hashing each file's path and content
/// in sorted order to ensure deterministic results. This is used for macOS
/// .app bundles which are directories, not single files.
///
/// # Algorithm
///
/// 1. Recursively collect all files using walkdir
/// 2. Sort paths lexicographically for deterministic order
/// 3. For each file: hash(relative_path + file_content)
/// 4. Return final combined hash
///
/// # Arguments
///
/// * `dir_path` - Path to directory to hash
///
/// # Returns
///
/// * `Ok(String)` - Hex-encoded SHA-256 hash of entire directory tree
/// * `Err` - If directory cannot be traversed
async fn calculate_directory_sha256(dir_path: &std::path::Path) -> Result<String> {
    // Collect all files recursively
    let mut entries: Vec<_> = walkdir::WalkDir::new(dir_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    // Sort by path for deterministic ordering
    entries.sort_by_key(|e| e.path().to_path_buf());

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];

    for entry in entries {
        // Include relative path in hash (preserves directory structure)
        if let Ok(rel_path) = entry.path().strip_prefix(dir_path) {
            hasher.update(rel_path.to_string_lossy().as_bytes());
        }

        // Hash file content
        let mut file = tokio::fs::File::open(entry.path())
            .await
            .fs_context("opening file for hashing", entry.path())?;

        loop {
            let n = file
                .read(&mut buffer)
                .await
                .fs_context("reading file for hash calculation", entry.path())?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}
