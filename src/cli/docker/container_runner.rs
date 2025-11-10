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

    /// Builds Docker command arguments for end-to-end bundling.
    ///
    /// Container receives source and output path, clones internally,
    /// builds, and writes artifact to mounted output directory.
    ///
    /// # Arguments
    ///
    /// * `container_name` - Unique container name
    /// * `source` - Source specification (unchanged from user input)
    /// * `output_path` - Final output path on host
    /// * `platform` - Platform to bundle
    ///
    /// # Returns
    ///
    /// Vector of command arguments for `docker run`
    pub fn build_docker_args_for_full_bundle(
        &self,
        container_name: &str,
        source: &str,
        output_path: &Path,
        platform: PackageType,
    ) -> Vec<String> {
        let platform_str = super::platform::platform_type_to_string(platform);

        // Extract output filename
        let output_filename = output_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("output.bin");

        // Mount output directory (self.workspace_path is actually output_parent in new flow)
        let output_mount = format!("{}:/output:rw", self.workspace_path.display());

        let mut docker_args = vec![
            "run".to_string(),
            "--name".to_string(),
            container_name.to_string(),
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
            // Mount output directory
            "-v".to_string(),
            output_mount,
            // Working directory in /tmp (not /workspace)
            "-w".to_string(),
            "/tmp/kodegen-build".to_string(),
            // Environment
            "-e".to_string(),
            "CARGO_HOME=/tmp/cargo".to_string(),
        ];

        // Image runs as builder user (UID 1000, GID 1000) by default
        // No --user flag needed

        // Image and command
        docker_args.push(self.image_name.clone());
        docker_args.push("kodegen_bundler_bundle".to_string());
        docker_args.push("--source".to_string());
        docker_args.push(source.to_string());
        docker_args.push("--platform".to_string());
        docker_args.push(platform_str.to_string());
        docker_args.push("--output-binary".to_string());
        docker_args.push(format!("/output/{}", output_filename));

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
                        runtime_config.indent(&line).expect("Failed to write docker output");
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
                )).expect("Failed to write to stdout");

                if let Err(e) = child.kill().await {
                    runtime_config.warn(&format!("Failed to kill docker run process: {}", e)).expect("Failed to write to stdout");
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
