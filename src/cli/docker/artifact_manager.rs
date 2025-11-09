//! Artifact discovery, validation, and file management for bundled packages.

use super::artifacts::verify_artifacts;
use crate::bundler::PackageType;
use crate::error::{BundlerError, CliError};
use std::path::{Path, PathBuf};

/// Artifact manager for handling bundle outputs.
pub struct ArtifactManager {
    workspace_path: PathBuf,
}

impl ArtifactManager {
    /// Creates a new artifact manager.
    ///
    /// # Arguments
    ///
    /// * `workspace_path` - Root path of the workspace
    pub fn new(workspace_path: PathBuf) -> Self {
        Self { workspace_path }
    }

    /// Discovers artifacts in the bundle directory.
    ///
    /// # Arguments
    ///
    /// * `temp_target_dir` - Temporary target directory containing build outputs
    /// * `platform` - Platform type to filter artifacts
    /// * `runtime_config` - Runtime configuration for verbose logging
    ///
    /// # Returns
    ///
    /// Vector of artifact paths
    pub async fn discover_artifacts(
        &self,
        temp_target_dir: &Path,
        platform: PackageType,
        runtime_config: &crate::cli::RuntimeConfig,
    ) -> Result<Vec<PathBuf>, BundlerError> {
        let platform_str = super::platform::platform_type_to_string(platform);
        let bundle_dir = temp_target_dir
            .join("release")
            .join("bundle")
            .join(platform_str.to_lowercase());

        if !tokio::fs::try_exists(&bundle_dir).await.unwrap_or(false) {
            return Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "find bundle directory".to_string(),
                reason: format!(
                    "Bundle directory not found: {}\nExpected artifacts from container build",
                    bundle_dir.display()
                ),
            }));
        }

        runtime_config.verbose_println(&format!(
            "Scanning for artifacts in: {}",
            bundle_dir.display()
        ));

        // Use spawn_blocking for complex directory iteration with filtering
        let artifacts = {
            let bundle_dir = bundle_dir.clone();
            let runtime_config = runtime_config.clone();
            tokio::task::spawn_blocking(move || {
                let entries = std::fs::read_dir(&bundle_dir).map_err(|e| {
                    BundlerError::Cli(CliError::ExecutionFailed {
                        command: "read bundle directory".to_string(),
                        reason: format!("Failed to read {}: {}", bundle_dir.display(), e),
                    })
                })?;

                let mut artifacts = Vec::new();
                for entry in entries {
                    let entry = entry.map_err(|e| {
                        BundlerError::Cli(CliError::ExecutionFailed {
                            command: "read directory entry".to_string(),
                            reason: format!("Failed to read entry in {}: {}", bundle_dir.display(), e),
                        })
                    })?;
                    let path = entry.path();

                    // Skip non-regular files (directories, symlinks)
                    let metadata = std::fs::symlink_metadata(&path).map_err(|e| {
                        BundlerError::Cli(CliError::ExecutionFailed {
                            command: "read file metadata".to_string(),
                            reason: format!("Failed to read metadata for {}: {}", path.display(), e),
                        })
                    })?;

                    if !metadata.is_file() || metadata.is_symlink() {
                        runtime_config.verbose_println(&format!("  Skipping non-regular file: {}", path.display()));
                        continue;
                    }

                    // Check minimum size
                    if metadata.len() < 1024 {
                        runtime_config.verbose_println(&format!(
                            "  Skipping small file: {} ({} bytes)",
                            path.display(),
                            metadata.len()
                        ));
                        continue;
                    }

                    // Validate file extension matches platform
                    let extension = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase());

                    let is_valid = match platform {
                        PackageType::Deb => extension.as_deref() == Some("deb"),
                        PackageType::Rpm => extension.as_deref() == Some("rpm"),
                        PackageType::AppImage => {
                            extension.is_none() || extension.as_deref() == Some("appimage")
                        }
                        PackageType::Nsis => extension.as_deref() == Some("exe"),
                        PackageType::Dmg => extension.as_deref() == Some("dmg"),
                        PackageType::MacOsBundle => extension.as_deref() == Some("app"),
                    };

                    if is_valid {
                        runtime_config.verbose_println(&format!("  ✓ Artifact: {}", path.display()));
                        artifacts.push(path);
                    } else {
                        runtime_config.verbose_println(&format!(
                            "  Skipping non-artifact: {} (wrong extension)",
                            path.display()
                        ));
                    }
                }

                Ok::<Vec<PathBuf>, BundlerError>(artifacts)
            })
            .await
            .map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "discover_artifacts".to_string(),
                    reason: format!("Task panicked: {}", e),
                })
            })??
        };

        runtime_config.verbose_println(&format!("Collected {} artifact(s)", artifacts.len()));

        if artifacts.is_empty() {
            return Err(self.format_no_artifacts_error(&bundle_dir, platform).await?);
        }

        Ok(artifacts)
    }

    /// Formats error when no artifacts are found.
    async fn format_no_artifacts_error(
        &self,
        bundle_dir: &Path,
        platform: PackageType,
    ) -> Result<BundlerError, BundlerError> {
        let platform_str = super::platform::platform_type_to_string(platform);

        let dir_contents = {
            let bundle_dir = bundle_dir.to_path_buf();
            tokio::task::spawn_blocking(move || {
                match std::fs::read_dir(&bundle_dir) {
                    Ok(entries) => {
                        let items: Vec<_> = entries
                            .flatten()
                            .map(|e| {
                                let path = e.path();
                                let name = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("<unknown>");
                                if path.is_dir() {
                                    format!("  [DIR]  {}", name)
                                } else {
                                    let size = path.metadata().ok().map(|m| m.len()).unwrap_or(0);
                                    format!("  [FILE] {} ({} bytes)", name, size)
                                }
                            })
                            .collect();
                        if items.is_empty() {
                            None
                        } else {
                            Some(items.join("\n"))
                        }
                    }
                    Err(e) => Some(format!("[Cannot read directory: {}]", e)),
                }
            })
            .await
            .unwrap_or_else(|_| Some("[Task panicked while reading directory]".to_string()))
        };

        let reason = match dir_contents {
            Some(contents) => format!(
                "No artifact files found matching expected patterns in:\n\
                 {}\n\
                 \n\
                 Directory contents:\n\
                 {}\n\
                 \n\
                 Expected artifacts like:\n\
                 • {}.deb (Debian package)\n\
                 • {}.rpm (RedHat package)\n\
                 • {}.AppImage (AppImage bundle)\n\
                 etc.",
                bundle_dir.display(),
                contents,
                platform_str,
                platform_str,
                platform_str
            ),
            None => format!(
                "Bundle directory is empty or inaccessible:\n\
                 {}\n\
                 \n\
                 Possible causes:\n\
                 • Bundle command failed silently inside container\n\
                 • Incorrect output directory path\n\
                 • Permission issues\n\
                 \n\
                 Check container logs:\n\
                 docker ps -a | head -2",
                bundle_dir.display()
            ),
        };

        Ok(BundlerError::Cli(CliError::ExecutionFailed {
            command: "find artifacts".to_string(),
            reason,
        }))
    }

    /// Atomically moves artifacts from temporary to final location.
    ///
    /// # Arguments
    ///
    /// * `artifacts` - Artifact paths in temporary location
    /// * `temp_target_dir` - Temporary target directory
    /// * `platform` - Platform type
    /// * `runtime_config` - Runtime configuration
    ///
    /// # Returns
    ///
    /// Updated artifact paths in final location
    pub async fn move_artifacts_to_final(
        &self,
        artifacts: Vec<PathBuf>,
        temp_target_dir: &PathBuf,
        platform: PackageType,
        runtime_config: &crate::cli::RuntimeConfig,
    ) -> Result<Vec<PathBuf>, BundlerError> {
        let platform_str = super::platform::platform_type_to_string(platform);

        // Verify artifacts before moving
        verify_artifacts(&artifacts, runtime_config)?;

        let final_bundle_dir = self
            .workspace_path
            .join("target")
            .join("release")
            .join("bundle")
            .join(platform_str.to_lowercase());

        let temp_bundle_dir = temp_target_dir
            .join("release")
            .join("bundle")
            .join(platform_str.to_lowercase());

        // Ensure parent directory exists
        if let Some(parent) = final_bundle_dir.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "create bundle parent directory".to_string(),
                    reason: format!("Failed to create {}: {}", parent.display(), e),
                })
            })?;
        }

        // Platform-specific atomic artifact replacement
        // Unix: rename() atomically replaces target, preventing race conditions
        // Windows: rename() fails if target exists, requires explicit removal
        #[cfg(unix)]
        {
            // Unix: Direct atomic rename with replacement
            // This prevents race conditions where concurrent builds or interruptions
            // could leave the bundle directory in an inconsistent state
            tokio::fs::rename(&temp_bundle_dir, &final_bundle_dir).await.map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "move artifacts to final location".to_string(),
                    reason: format!(
                        "Failed to rename {} to {}: {}",
                        temp_bundle_dir.display(),
                        final_bundle_dir.display(),
                        e
                    ),
                })
            })?;
        }

        #[cfg(windows)]
        {
            // Windows: rename() fails if target exists, must remove first
            // Note: This creates a small race window on Windows, but is acceptable
            // as the bundler typically runs on Linux for cross-platform builds
            match tokio::fs::try_exists(&final_bundle_dir).await {
                Ok(true) => {
                    tokio::fs::remove_dir_all(&final_bundle_dir).await.map_err(|e| {
                        BundlerError::Cli(CliError::ExecutionFailed {
                            command: "remove old bundle directory".to_string(),
                            reason: format!("Failed to remove {}: {}", final_bundle_dir.display(), e),
                        })
                    })?;
                }
                Ok(false) => {
                    // Directory doesn't exist, no need to remove
                }
                Err(_) => {
                    // If we can't check existence, proceed with rename anyway
                    // The rename operation will fail with a clear error if needed
                }
            }
            
            tokio::fs::rename(&temp_bundle_dir, &final_bundle_dir).await.map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "move artifacts to final location".to_string(),
                    reason: format!(
                        "Failed to move {} to {}: {}",
                        temp_bundle_dir.display(),
                        final_bundle_dir.display(),
                        e
                    ),
                })
            })?;
        }

        runtime_config.verbose_println(&format!(
            "Moved artifacts from temp to final location: {}",
            final_bundle_dir.display()
        ));

        // Update artifact paths to point to final location
        let artifacts = artifacts
            .into_iter()
            .map(|path| {
                path.strip_prefix(temp_target_dir)
                    .ok()
                    .map(|rel| self.workspace_path.join("target").join(rel))
                    .unwrap_or(path)
            })
            .collect::<Vec<_>>();

        Ok(artifacts)
    }

    /// Cleans up temporary target directory.
    ///
    /// # Arguments
    ///
    /// * `temp_target_dir` - Temporary directory to remove
    /// * `runtime_config` - Runtime configuration for logging
    pub async fn cleanup_temp_directory(
        &self,
        temp_target_dir: &PathBuf,
        runtime_config: &crate::cli::RuntimeConfig,
    ) {
        runtime_config.verbose_println(&format!(
            "Cleaning up temporary target directory: {}",
            temp_target_dir.display()
        ));

        if let Err(e) = tokio::fs::remove_dir_all(temp_target_dir).await {
            runtime_config.warn(&format!(
                "Failed to cleanup temp directory {}: {}",
                temp_target_dir.display(),
                e
            ));
        }
    }
}
