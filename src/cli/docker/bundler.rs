//! Docker container bundler for cross-platform builds.
#![allow(dead_code)] // Public API - items may be used by external consumers

//!
//! Manages Docker container lifecycle for building packages on platforms
//! other than the host OS.

use super::artifact_manager::ArtifactManager;
use super::container_runner::ContainerRunner;
use super::guard::ContainerGuard;
use super::limits::ContainerLimits;
use super::oom_detector::OomDetector;
use super::platform::platform_emoji;
use crate::bundler::PackageType;
use crate::error::BundlerError;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

/// Docker container bundler for cross-platform builds.
///
/// Manages Docker container lifecycle for building packages on platforms
/// other than the host OS.
#[derive(Debug)]
pub struct ContainerBundler {
    image_name: String,
    workspace_path: PathBuf,
    pub limits: ContainerLimits,
}

impl ContainerBundler {
    /// Creates a container bundler with custom resource limits.
    ///
    /// # Arguments
    ///
    /// * `workspace_path` - Path to the workspace root (will be mounted in container)
    /// * `limits` - Resource limits for the container
    pub fn with_limits(workspace_path: PathBuf, limits: ContainerLimits) -> Self {
        Self {
            image_name: super::image::BUILDER_IMAGE_NAME.to_string(),
            workspace_path,
            limits,
        }
    }

    /// Bundles a single platform in a Docker container.
    ///
    /// Runs the pre-built bundler binary inside the container, which builds binaries
    /// and creates the package artifact.
    ///
    /// # Arguments
    ///
    /// * `platform` - The package type to build
    /// * `binary_name` - Name of the binary to bundle
    /// * `version` - Version string for the package
    /// * `runtime_config` - Runtime configuration for output
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<PathBuf>)` - Paths to created artifacts
    /// * `Err` - Container execution failed
    pub async fn bundle_platform(
        &self,
        platform: PackageType,
        binary_name: &str,
        runtime_config: &crate::cli::RuntimeConfig,
    ) -> Result<Vec<PathBuf>, BundlerError> {
        let platform_str = super::platform::platform_type_to_string(platform);

        runtime_config.indent(&format!(
            "{} Building {} package in container...",
            platform_emoji(platform),
            platform_str
        ));

        // Generate UUID for both container name AND temp directory
        let build_uuid = Uuid::new_v4();
        let container_name = format!("kodegen-bundle-{}", build_uuid);

        // Create RAII guard to ensure cleanup on failure
        let _guard = ContainerGuard {
            name: container_name.clone(),
        };

        // Resolve and validate workspace path
        let workspace_path = self.resolve_workspace_path()?;
        let temp_target_dir = self.prepare_temp_directory(&workspace_path, &build_uuid)?;

        // Create container runner and build Docker arguments
        let runner = ContainerRunner::new(
            self.image_name.clone(),
            workspace_path.clone(),
            self.limits.memory.clone(),
            self.limits.memory_swap.clone(),
            self.limits.cpus.clone(),
            self.limits.pids_limit,
        );

        let docker_args = runner.build_docker_args(
            &container_name,
            &temp_target_dir,
            platform,
            binary_name,
        );

        // Run container and capture output
        let result = runner.run_container(docker_args, runtime_config).await?;

        // Check for OOM or other failures
        if !result.status.success() {
            return self
                .handle_container_failure(
                    platform,
                    result.status.code().unwrap_or(-1),
                    &result.stderr_lines,
                    &container_name,
                )
                .await;
        }

        runtime_config.indent(&format!("✓ Created {} package", platform_str));

        // Discover and move artifacts
        let artifact_mgr = ArtifactManager::new(workspace_path.clone());
        let artifacts =
            artifact_mgr.discover_artifacts(&temp_target_dir, platform, runtime_config).await?;
        let artifacts = artifact_mgr.move_artifacts_to_final(
            artifacts,
            &temp_target_dir,
            platform,
            runtime_config,
        ).await?;

        // Clean up temporary directory
        artifact_mgr.cleanup_temp_directory(&temp_target_dir, runtime_config).await;

        Ok(artifacts)
    }

    /// Resolves and validates the workspace path.
    fn resolve_workspace_path(&self) -> Result<PathBuf, BundlerError> {
        use crate::error::CliError;

        let workspace_path = self
            .workspace_path
            .canonicalize()
            .or_else(|_| {
                if self.workspace_path.is_absolute() {
                    Ok(self.workspace_path.clone())
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(&self.workspace_path))
                        .map_err(|e| {
                            std::io::Error::other(format!(
                                "Cannot determine current directory: {}",
                                e
                            ))
                        })
                }
            })
            .map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "resolve workspace path".to_string(),
                    reason: format!(
                        "Cannot resolve workspace path '{}': {}\n\
                         \n\
                         Ensure the path exists and is accessible.",
                        self.workspace_path.display(),
                        e
                    ),
                })
            })?;

        // SECURITY: Verify it's actually a directory
        if !workspace_path.is_dir() {
            return Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "validate workspace".to_string(),
                reason: format!(
                    "Workspace path is not a directory: {}\n\
                     \n\
                     The bundle command requires a valid Cargo workspace directory.\n\
                     Check that the path points to a directory containing Cargo.toml.",
                    workspace_path.display()
                ),
            }));
        }

        Ok(workspace_path)
    }

    /// Prepares temporary target directory for isolated build.
    fn prepare_temp_directory(
        &self,
        workspace_path: &Path,
        build_uuid: &Uuid,
    ) -> Result<PathBuf, BundlerError> {
        use crate::error::CliError;

        // Ensure main target directory exists
        let target_dir = workspace_path.join("target");
        std::fs::create_dir_all(&target_dir).map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "create target directory".to_string(),
                reason: format!(
                    "Failed to ensure target directory exists: {}\n\
                     Path: {}\n\
                     This directory is required for build outputs.\n\
                     \n\
                     Check that:\n\
                     • You have write permissions to the workspace\n\
                     • The filesystem is not read-only\n\
                     • There's sufficient disk space",
                    e,
                    target_dir.display()
                ),
            })
        })?;

        // Create isolated temp target directory for this build
        let temp_target_dir = workspace_path.join(format!("target-temp-{}", build_uuid));
        std::fs::create_dir_all(&temp_target_dir).map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "create temporary target directory".to_string(),
                reason: format!("Failed to create {}: {}", temp_target_dir.display(), e),
            })
        })?;

        Ok(temp_target_dir)
    }

    /// Handles container execution failures with OOM detection.
    async fn handle_container_failure(
        &self,
        platform: PackageType,
        exit_code: i32,
        stderr_lines: &[String],
        container_name: &str,
    ) -> Result<Vec<PathBuf>, BundlerError> {
        let detector =
            OomDetector::new(self.limits.memory.clone(), self.limits.memory_swap.clone());

        if detector
            .is_oom_failure(exit_code, stderr_lines, container_name)
            .await
        {
            Err(detector
                .format_oom_error(platform, stderr_lines, exit_code, container_name)
                .await)
        } else if exit_code == 137 {
            // Special handling for SIGKILL without OOM evidence
            Err(detector.format_sigkill_error(platform, stderr_lines))
        } else {
            Err(detector.format_generic_error(platform, exit_code, stderr_lines))
        }
    }
}
