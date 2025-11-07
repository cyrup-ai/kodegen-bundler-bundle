//! Docker container execution and process management.

use crate::bundler::PackageType;
use crate::error::{BundlerError, CliError};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Timeout for Docker container run operations (20 minutes)
/// Container bundling involves full cargo builds which can be slow
pub const DOCKER_RUN_TIMEOUT: Duration = Duration::from_secs(1200);

/// Result of container execution
pub struct ContainerRunResult {
    /// Exit status of the container
    pub status: std::process::ExitStatus,
    /// Captured stderr lines
    pub stderr_lines: Vec<String>,
}

/// Docker container runner for executing bundling operations.
pub struct ContainerRunner {
    image_name: String,
    workspace_path: PathBuf,
    memory_limit: String,
    memory_swap: String,
    cpus_limit: String,
    pids_limit: u32,
}

impl ContainerRunner {
    /// Creates a new container runner.
    ///
    /// # Arguments
    ///
    /// * `image_name` - Docker image to use
    /// * `workspace_path` - Path to workspace (must be absolute)
    /// * `memory_limit` - Memory limit (e.g., "4g")
    /// * `memory_swap` - Memory + swap limit (e.g., "8g")
    /// * `cpus_limit` - CPU limit (e.g., "2.0")
    /// * `pids_limit` - Maximum PIDs
    pub fn new(
        image_name: String,
        workspace_path: PathBuf,
        memory_limit: String,
        memory_swap: String,
        cpus_limit: String,
        pids_limit: u32,
    ) -> Self {
        Self {
            image_name,
            workspace_path,
            memory_limit,
            memory_swap,
            cpus_limit,
            pids_limit,
        }
    }

    /// Builds Docker command arguments for container execution.
    ///
    /// # Arguments
    ///
    /// * `container_name` - Unique container name
    /// * `temp_target_dir` - Temporary target directory path
    /// * `platform` - Platform to bundle
    /// * `binary_name` - Name of binary
    ///
    /// # Returns
    ///
    /// Vector of command arguments for `docker run`
    pub fn build_docker_args(
        &self,
        container_name: &str,
        temp_target_dir: &Path,
        platform: PackageType,
        binary_name: &str,
    ) -> Vec<String> {
        let platform_str = super::platform::platform_type_to_string(platform);

        // SECURITY: Build secure mount arguments
        let workspace_mount = format!("{}:/workspace:ro", self.workspace_path.display());
        let target_mount = format!("{}:/workspace/target:rw", temp_target_dir.display());

        let mut docker_args = vec![
            "run".to_string(),
            "--name".to_string(),
            container_name.to_string(),
            // Note: No --rm flag - ContainerGuard handles cleanup after OOM check
            // SECURITY: Prevent privilege escalation in container
            "--security-opt".to_string(),
            "no-new-privileges".to_string(),
            // SECURITY: Drop all capabilities
            "--cap-drop".to_string(),
            "ALL".to_string(),
            // Memory limits
            "--memory".to_string(),
            self.memory_limit.clone(),
            "--memory-swap".to_string(),
            self.memory_swap.clone(),
            // CPU limits
            "--cpus".to_string(),
            self.cpus_limit.clone(),
            // Process limits
            "--pids-limit".to_string(),
            self.pids_limit.to_string(),
            // SECURITY: Mount workspace read-only
            "-v".to_string(),
            workspace_mount,
            // SECURITY: Mount target/ read-write for build outputs
            "-v".to_string(),
            target_mount,
            // Set working directory
            "-w".to_string(),
            "/workspace".to_string(),
        ];

        // User mapping for file ownership (Unix only)
        #[cfg(unix)]
        {
            let uid = users::get_current_uid();
            let gid = users::get_current_gid();
            docker_args.push("--user".to_string());
            docker_args.push(format!("{}:{}", uid, gid));
        }

        // Add image and bundler command
        docker_args.push(self.image_name.clone());
        docker_args.push("kodegen_bundler_bundle".to_string());
        docker_args.push("--repo-path".to_string());
        docker_args.push("/workspace".to_string());
        docker_args.push("--platform".to_string());
        docker_args.push(platform_str.to_string());
        docker_args.push("--binary-name".to_string());
        docker_args.push(binary_name.to_string());

        docker_args
    }

    /// Runs a Docker container and streams output.
    ///
    /// # Arguments
    ///
    /// * `docker_args` - Docker command arguments
    /// * `runtime_config` - Runtime configuration for output
    ///
    /// # Returns
    ///
    /// `ContainerRunResult` with exit status and stderr lines
    pub async fn run_container(
        &self,
        docker_args: Vec<String>,
        runtime_config: &crate::cli::RuntimeConfig,
    ) -> Result<ContainerRunResult, BundlerError> {
        // Spawn docker process with both stdout/stderr piped
        let mut child = Command::new("docker")
            .args(&docker_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: format!("docker run {}", docker_args.join(" ")),
                    reason: e.to_string(),
                })
            })?;

        // Process both stdout and stderr concurrently to avoid race conditions
        // Both streams must complete before we check exit status
        let (_, stderr_result) = tokio::join!(
            // Process stdout: stream in real-time
            async {
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();

                    while let Ok(Some(line)) = lines.next_line().await {
                        runtime_config.indent(&line);
                    }
                }
            },
            // Process stderr: capture for OOM detection
            async {
                if let Some(stderr) = child.stderr.take() {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    let mut captured_lines = Vec::new();

                    while let Ok(Some(line)) = lines.next_line().await {
                        captured_lines.push(line);
                    }

                    Some(captured_lines)
                } else {
                    None
                }
            }
        );

        // Wait for child process completion with timeout
        let status = tokio::time::timeout(DOCKER_RUN_TIMEOUT, child.wait()).await;

        let status = match status {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => {
                return Err(BundlerError::Cli(CliError::ExecutionFailed {
                    command: format!("docker run {}", docker_args.join(" ")),
                    reason: e.to_string(),
                }));
            }
            Err(_elapsed) => {
                // Timeout - kill the process
                runtime_config.warn(&format!(
                    "Docker bundling timed out after {} minutes, terminating...",
                    DOCKER_RUN_TIMEOUT.as_secs() / 60
                ));

                if let Err(e) = child.kill().await {
                    eprintln!("Warning: Failed to kill docker run process: {}", e);
                }

                let _ = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;

                return Err(BundlerError::Cli(CliError::ExecutionFailed {
                    command: "docker run".to_string(),
                    reason: format!(
                        "Docker bundling timed out after {} minutes.\n\
                         \n\
                         This usually indicates:\n\
                         • Very slow build (large dependency downloads)\n\
                         • System resource constraints\n\
                         • Network issues\n\
                         \n\
                         Try:\n\
                         • Increase container resource limits\n\
                         • Check available system memory/CPU\n\
                         • Use --build flag to reuse cached builds",
                        DOCKER_RUN_TIMEOUT.as_secs() / 60
                    ),
                }));
            }
        };

        // Extract captured stderr lines (both streams already completed via tokio::join!)
        let stderr_lines = stderr_result.unwrap_or_default();

        Ok(ContainerRunResult {
            status,
            stderr_lines,
        })
    }
}
