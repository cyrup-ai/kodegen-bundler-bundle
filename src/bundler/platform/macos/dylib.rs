//! Dynamic library dependency discovery and bundling for macOS .app bundles.
//!
//! This module handles automatic discovery of non-system dylib dependencies and bundles
//! them into the .app's Frameworks directory, rewriting load paths to use @rpath.

use crate::bundler::{
    error::{ErrorExt, Result},
    settings::Settings,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs as tokio_fs;

/// Bundles all dynamic library dependencies for binaries in the .app bundle.
///
/// This function:
/// 1. Discovers non-system dylib dependencies for each binary
/// 2. Recursively bundles those dylibs and their dependencies into Contents/Frameworks/
/// 3. Rewrites binary load paths to use @rpath
/// 4. Adds @rpath pointing to @executable_path/../Frameworks
///
/// # Arguments
/// * `macos_dir` - Path to Contents/MacOS directory
/// * `contents_dir` - Path to Contents directory
/// * `settings` - Bundle configuration
pub async fn bundle_dylib_dependencies(
    macos_dir: &Path,
    contents_dir: &Path,
    settings: &Settings,
) -> Result<()> {
    let frameworks_dir = contents_dir.join("Frameworks");

    // Create Frameworks directory if it doesn't exist
    if !frameworks_dir.exists() {
        tokio_fs::create_dir_all(&frameworks_dir)
            .await
            .fs_context("failed to create Frameworks directory", &frameworks_dir)?;
    }

    // Track processed dylibs across all binaries to avoid duplicates
    let mut processed = HashSet::new();

    // Process each binary in the bundle
    for binary in settings.binaries() {
        let binary_path = if binary.main() {
            macos_dir.join(binary.name())
        } else {
            contents_dir.join("Resources").join(binary.name())
        };

        log::info!("Discovering dylib dependencies for {}", binary.name());

        // Get non-system dylib dependencies
        let deps = get_dylib_dependencies(&binary_path)?;
        let non_system: Vec<String> = deps
            .into_iter()
            .filter(|d| !is_system_dylib(d))
            .collect();

        if !non_system.is_empty() {
            log::info!("Found {} non-system dylibs for {}", non_system.len(), binary.name());
            for dylib in &non_system {
                log::debug!("  - {}", dylib);
            }
        }

        // Bundle each non-system dylib recursively
        for dylib_path_str in non_system {
            let dylib_path = resolve_dylib_path(&dylib_path_str)?;
            bundle_dylib_and_deps(&dylib_path, &frameworks_dir, &mut processed).await?;
        }

        // Fix binary's load paths to use @rpath
        if !processed.is_empty() {
            fix_binary_dylib_paths(&binary_path, &processed).await?;
        }
    }

    if !processed.is_empty() {
        log::info!("Bundled {} unique dylibs into Frameworks/", processed.len());
    }

    Ok(())
}

/// Extracts dynamic library dependencies from a Mach-O binary using goblin.
///
/// Returns a list of dylib paths as strings.
fn get_dylib_dependencies(binary_path: &Path) -> Result<Vec<String>> {
    let buffer = std::fs::read(binary_path)
        .fs_context("failed to read binary", binary_path)?;

    match goblin::Object::parse(&buffer)
        .map_err(|e| crate::bundler::error::Error::GenericError(format!("failed to parse binary with goblin: {}", e)))?
    {
        goblin::Object::Mach(goblin::mach::Mach::Binary(macho)) => {
            Ok(macho.libs.iter().map(|s| s.to_string()).collect())
        }
        goblin::Object::Mach(goblin::mach::Mach::Fat(_fat)) => {
            // For fat binaries, parse the first architecture
            // All architectures should have the same dylib dependencies
            if let Ok(goblin::Object::Mach(goblin::mach::Mach::Binary(macho))) = goblin::Object::parse(&buffer) {
                return Ok(macho.libs.iter().map(|s| s.to_string()).collect());
            }
            Ok(vec![])
        }
        _ => {
            log::warn!("Binary {} is not a Mach-O file, skipping dylib discovery", binary_path.display());
            Ok(vec![])
        }
    }
}

/// Determines if a dylib path is a system library that should NOT be bundled.
///
/// System libraries include:
/// - /System/Library/ - macOS system frameworks
/// - /usr/lib/ - System libraries
/// - "self" - Mach-O special value indicating the binary itself
///
/// Non-system libraries include:
/// - /opt/homebrew/ - Homebrew packages
/// - /usr/local/Cellar/ - Homebrew Cellar
/// - Relative paths or @rpath (already bundled)
fn is_system_dylib(path: &str) -> bool {
    path == "self" ||
    path.starts_with("/System/") ||
    path.starts_with("/usr/lib/") ||
    path.starts_with("@rpath") ||
    path.starts_with("@executable_path") ||
    path.starts_with("@loader_path")
}

/// Resolves a dylib path string to an actual filesystem path.
///
/// Handles:
/// - Absolute paths: returned as-is
/// - Relative paths: error
/// - @rpath, @executable_path: error (should be filtered earlier)
fn resolve_dylib_path(path_str: &str) -> Result<PathBuf> {
    // Handle wildcard paths from Homebrew (e.g., /opt/homebrew/*/lib/libpcre2.dylib)
    if path_str.contains('*') {
        // Try to resolve the wildcard
        if let Some(resolved) = resolve_wildcard_path(path_str) {
            return Ok(resolved);
        }
        return Err(crate::bundler::error::Error::GenericError(format!(
            "Cannot resolve wildcard dylib path: {}",
            path_str
        )));
    }

    let path = PathBuf::from(path_str);

    if !path.is_absolute() {
        return Err(crate::bundler::error::Error::GenericError(format!(
            "Relative dylib path not supported: {}",
            path_str
        )));
    }

    if !path.exists() {
        return Err(crate::bundler::error::Error::GenericError(format!(
            "Dylib not found: {}",
            path_str
        )));
    }

    Ok(path)
}

/// Resolves wildcard paths by checking common locations.
///
/// For paths like "/opt/homebrew/*/lib/libpcre2.dylib", checks:
/// - /opt/homebrew/Cellar/*/lib/libpcre2.dylib
fn resolve_wildcard_path(path_str: &str) -> Option<PathBuf> {
    // Replace * with Cellar/* pattern for Homebrew
    if let Some(after_homebrew) = path_str.strip_prefix("/opt/homebrew/*/") {
        // Try Cellar pattern
        let pattern = format!("/opt/homebrew/Cellar/*/{}", after_homebrew);
        if let Ok(entries) = glob::glob(&pattern) {
            // Return first match
            for entry in entries.flatten() {
                if entry.exists() {
                    return Some(entry);
                }
            }
        }
    }
    None
}

/// Recursively bundles a dylib and its dependencies into the Frameworks directory.
///
/// # Arguments
/// * `dylib_path` - Path to the dylib to bundle
/// * `frameworks_dir` - Destination Frameworks directory
/// * `processed` - Set of already-processed dylib paths (to avoid duplicates)
async fn bundle_dylib_and_deps(
    dylib_path: &Path,
    frameworks_dir: &Path,
    processed: &mut HashSet<PathBuf>,
) -> Result<()> {
    // Skip if already processed
    if processed.contains(dylib_path) {
        return Ok(());
    }

    log::debug!("Bundling dylib: {}", dylib_path.display());

    // Mark as processed
    processed.insert(dylib_path.to_path_buf());

    // Get dylib filename
    let dylib_name = dylib_path.file_name()
        .ok_or_else(|| crate::bundler::error::Error::GenericError(format!(
            "Invalid dylib path: {}",
            dylib_path.display()
        )))?;

    // Copy dylib to Frameworks directory
    let dest_path = frameworks_dir.join(dylib_name);
    tokio_fs::copy(dylib_path, &dest_path)
        .await
        .fs_context("failed to copy dylib to Frameworks", dylib_path)?;

    // Get this dylib's dependencies
    let deps = get_dylib_dependencies(dylib_path)?;
    let non_system: Vec<String> = deps
        .into_iter()
        .filter(|d| !is_system_dylib(d))
        .collect();

    // Recursively bundle dependencies
    for dep_path_str in non_system {
        if let Ok(dep_path) = resolve_dylib_path(&dep_path_str) {
            Box::pin(bundle_dylib_and_deps(&dep_path, frameworks_dir, processed)).await?;
        }
    }

    // Fix this dylib's internal load paths
    fix_dylib_internal_paths(&dest_path, processed).await?;

    Ok(())
}

/// Rewrites a dylib's internal load paths to use @rpath.
///
/// This fixes the dylib's dependencies to point to @rpath instead of absolute paths.
async fn fix_dylib_internal_paths(
    dylib_path: &Path,
    _processed: &HashSet<PathBuf>,
) -> Result<()> {
    // Get dependencies
    let deps = get_dylib_dependencies(dylib_path)?;

    for dep in deps {
        if is_system_dylib(&dep) {
            continue; // Keep system libs as-is
        }

        // Extract filename from dependency path
        if let Some(filename) = PathBuf::from(&dep).file_name() {
            let new_path = format!("@rpath/{}", filename.to_string_lossy());

            // Use install_name_tool to rewrite the load path
            let status = Command::new("install_name_tool")
                .arg("-change")
                .arg(&dep)
                .arg(&new_path)
                .arg(dylib_path)
                .status()
                .fs_context("failed to run install_name_tool", dylib_path)?;

            if !status.success() {
                log::warn!("install_name_tool failed for {}: {} -> {}",
                    dylib_path.display(), dep, new_path);
            }
        }
    }

    Ok(())
}

/// Rewrites a binary's dylib load paths to use @rpath and adds rpath.
///
/// # Arguments
/// * `binary_path` - Path to the binary to fix
/// * `_processed` - Set of dylibs that were bundled (for filtering)
async fn fix_binary_dylib_paths(
    binary_path: &Path,
    _processed: &HashSet<PathBuf>,
) -> Result<()> {
    log::info!("Fixing dylib paths for {}", binary_path.display());

    // Get binary's dependencies
    let deps = get_dylib_dependencies(binary_path)?;

    // Rewrite each non-system dependency to use @rpath
    for dep in deps {
        if is_system_dylib(&dep) {
            continue;
        }

        // Extract filename
        if let Some(filename) = PathBuf::from(&dep).file_name() {
            let new_path = format!("@rpath/{}", filename.to_string_lossy());

            log::debug!("  Rewriting: {} -> {}", dep, new_path);

            let status = Command::new("install_name_tool")
                .arg("-change")
                .arg(&dep)
                .arg(&new_path)
                .arg(binary_path)
                .status()
                .fs_context("failed to run install_name_tool", binary_path)?;

            if !status.success() {
                return Err(crate::bundler::error::Error::GenericError(format!(
                    "install_name_tool failed for {}: {} -> {}",
                    binary_path.display(), dep, new_path
                )));
            }
        }
    }

    // Add rpath pointing to @executable_path/../Frameworks
    log::debug!("  Adding rpath: @executable_path/../Frameworks");

    let status = Command::new("install_name_tool")
        .arg("-add_rpath")
        .arg("@executable_path/../Frameworks")
        .arg(binary_path)
        .status()
        .fs_context("failed to add rpath", binary_path)?;

    if !status.success() {
        // This might fail if rpath already exists - that's OK
        log::debug!("  rpath may already exist (this is OK)");
    }

    Ok(())
}
