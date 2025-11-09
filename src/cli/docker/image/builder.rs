//! Docker image building operations.

use crate::error::{BundlerError, CliError};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::config::{BUILDER_IMAGE_NAME, DOCKER_BUILD_TIMEOUT};

/// Builds the Docker image from embedded Dockerfile.
///
/// # Arguments
///
/// * `docker_build_context` - Path to directory containing .devcontainer/Dockerfile
///   (typically a temp directory where embedded Dockerfile was extracted)
/// * `runtime_config` - Runtime configuration for output
///
/// # Returns
///
/// * `Ok(())` - Image built successfully
/// * `Err` - Build failed
pub async fn build_docker_image(
    docker_build_context: &Path,
    runtime_config: &crate::cli::RuntimeConfig,
) -> Result<(), BundlerError> {
    let dockerfile_dir = docker_build_context.join(".devcontainer");

    runtime_config.progress(&format!("Building Docker image: {}", BUILDER_IMAGE_NAME));

    // Spawn with piped stdout and stderr for streaming
    let mut child = Command::new("docker")
        .args([
            "build",
            "--pull",
            "-t",
            BUILDER_IMAGE_NAME,
            "-f",
            "Dockerfile",
            ".",
        ])
        .current_dir(&dockerfile_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "docker build".to_string(),
                reason: e.to_string(),
            })
        })?;

    // Stream both stdout and stderr concurrently through OutputManager
    tokio::join!(
        async {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    runtime_config.indent(&line);
                }
            }
        },
        async {
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    runtime_config.indent(&line);
                }
            }
        }
    );

    // Wait with timeout - handle timeout explicitly to kill child
    let status = tokio::time::timeout(DOCKER_BUILD_TIMEOUT, child.wait()).await;

    let status = match status {
        Ok(Ok(status)) => status, // Completed normally
        Ok(Err(e)) => {
            // Wait failed (process error)
            return Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "docker build".to_string(),
                reason: e.to_string(),
            }));
        }
        Err(_elapsed) => {
            // Timeout occurred - kill the process before returning error
            runtime_config.warn("Docker build timed out, terminating process...");

            // Kill process (SIGKILL)
            if let Err(e) = child.kill().await {
                runtime_config.warn(&format!("Failed to kill docker build process: {}", e));
            }

            // Wait for process to exit and reap zombie (with short timeout)
            let _ = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;

            return Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "docker build".to_string(),
                reason: format!(
                    "Docker build timed out after {} minutes.\n\
                     \n\
                     Possible causes:\n\
                     • Slow network connection to Docker registry\n\
                     • Large base image download\n\
                     • Complex Dockerfile with many layers\n\
                     \n\
                     Solutions:\n\
                     • Check network connection\n\
                     • Increase timeout if build is legitimately slow\n\
                     • Optimize Dockerfile (fewer layers, smaller base images)\n\
                     • Use local registry/cache",
                    DOCKER_BUILD_TIMEOUT.as_secs() / 60
                ),
            }));
        }
    };

    if !status.success() {
        return Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker build".to_string(),
            reason: format!(
                "Build failed with exit code: {}",
                status.code().unwrap_or(-1)
            ),
        }));
    }

    runtime_config.success("Docker image built successfully");
    Ok(())
}
