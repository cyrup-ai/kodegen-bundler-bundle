//! Docker container bundler for cross-platform builds.
#![allow(dead_code)] // Public API - items may be used by external consumers

//!
//! Manages Docker container lifecycle for building packages on platforms
//! other than the host OS.

use super::container_runner::ContainerRunner;
use super::guard::ContainerGuard;
use super::limits::ContainerLimits;
use super::oom_detector::OomDetector;
use super::platform::platform_emoji;
use crate::bundler::PackageType;
use crate::error::BundlerError;
use std::path::PathBuf;
use uuid::Uuid;

/// Docker container bundler for cross-platform builds.
///
/// Manages Docker container lifecycle for building packages on platforms
/// other than the host OS.
#[derive(Debug)]
pub struct ContainerBundler {
    image_name: String,
    source: String,
    output_path: PathBuf,
    pub limits: ContainerLimits,
}

impl ContainerBundler {
    /// Creates a container bundler for end-to-end bundling.
    ///
    /// The container will clone, build, and bundle internally, writing the
    /// artifact to the specified output path.
    ///
    /// # Arguments
    ///
    /// * `source` - Source specification (local path, GitHub org/repo, or GitHub URL)
    /// * `output_path` - Path where the bundled artifact should be written
    /// * `limits` - Resource limits for the container
    pub fn new(source: String, output_path: PathBuf, limits: ContainerLimits) -> Self {
        Self {
            image_name: super::image::BUILDER_IMAGE_NAME.to_string(),
            source,
            output_path,
            limits,
        }
    }

    /// Bundles a package in a Docker container (end-to-end).
    ///
    /// The container receives the source and output path, then:
    /// 1. Clones the repository
    /// 2. Builds the binary
    /// 3. Creates the package
    /// 4. Writes to the output path (via mounted directory)
    ///
    /// # Arguments
    ///
    /// * `platform` - The package type to build
    /// * `runtime_config` - Runtime configuration for output
    ///
    /// # Returns
    ///
    /// * `Ok(PathBuf)` - Path to created artifact (same as self.output_path)
    /// * `Err` - Container execution failed
    pub async fn bundle(
        &self,
        platform: PackageType,
        runtime_config: &crate::cli::RuntimeConfig,
    ) -> Result<PathBuf, BundlerError> {
        let platform_str = super::platform::platform_type_to_string(platform);

        runtime_config.indent(&format!(
            "{} Building {} package in container...",
            platform_emoji(platform),
            platform_str
        )).expect("Failed to write to stdout");

        // Generate UUID for container name
        let build_uuid = Uuid::new_v4();
        let container_name = format!("kodegen-bundle-{}", build_uuid);

        // Create RAII guard to ensure cleanup on failure
        let _guard = ContainerGuard {
            name: container_name.clone(),
            output: runtime_config.output().clone(),
        };

        // Create temp output directory on host
        let output_parent = self.output_path.parent().ok_or_else(|| {
            use crate::error::CliError;
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "determine output directory".to_string(),
                reason: format!("Output path has no parent directory: {}", self.output_path.display()),
            })
        })?;

        std::fs::create_dir_all(output_parent).map_err(|e| {
            use crate::error::CliError;
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "create output directory".to_string(),
                reason: format!("Failed to create {}: {}", output_parent.display(), e),
            })
        })?;

        // Create container runner
        let runner = ContainerRunner::new(
            self.image_name.clone(),
            output_parent.to_path_buf(),
            self.limits.memory.clone(),
            self.limits.memory_swap.clone(),
            self.limits.cpus.clone(),
            self.limits.pids_limit,
        );

        let docker_args = runner.build_docker_args_for_full_bundle(
            &container_name,
            &self.source,
            &self.output_path,
            platform,
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
                .await
                .map(|_| unreachable!());
        }

        runtime_config.indent(&format!("✓ Created {} package", platform_str)).expect("Failed to write to stdout");

        Ok(self.output_path.clone())
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
