//! Docker image management and orchestration.

use crate::error::{BundlerError, CliError};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use super::builder::build_docker_image;
use super::config::BUILDER_IMAGE_NAME;
use super::staleness::{get_image_age_days, is_image_up_to_date};

/// Checks if Docker daemon is responsive.
///
/// Performs a fast pre-flight check using `docker version` to verify the Docker daemon
/// is running and responsive. This prevents hangs when the daemon is deadlocked or
/// in an unresponsive state.
///
/// # Returns
///
/// * `Ok(())` - Docker daemon is responsive
/// * `Err` - Docker daemon is not responding, not installed, or hung
async fn check_docker_responsive() -> Result<(), BundlerError> {
    // Use 'docker version' which is faster and simpler than 'images'
    let result = timeout(
        Duration::from_secs(3), // Very short timeout
        Command::new("docker")
            .args(["version", "--format", "{{.Server.Version}}"])
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "docker version".to_string(),
                reason: format!(
                    "Docker daemon is not responding correctly:\n{}",
                    stderr
                ),
            }))
        }
        Ok(Err(e)) => Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker version".to_string(),
            reason: format!(
                "Cannot execute docker command: {}\n\
                 \n\
                 Possible causes:\n\
                 • Docker is not installed\n\
                 • Docker daemon is not running\n\
                 • Docker is not in PATH\n\
                 \n\
                 Try: docker version",
                e
            ),
        })),
        Err(_) => Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker version".to_string(),
            reason: "Docker health check timed out after 3 seconds.\n\
                     \n\
                     The Docker daemon appears to be hung or unresponsive.\n\
                     \n\
                     Troubleshooting:\n\
                     • Check: docker ps\n\
                     • Restart Docker daemon: sudo systemctl restart docker\n\
                     • Check logs: journalctl -u docker.service"
                .to_string(),
        })),
    }
}

/// Ensures the builder Docker image is built and up-to-date.
///
/// Checks if the image exists and whether it's stale (Dockerfile modified after image creation).
/// Automatically rebuilds if Dockerfile is newer than image.
///
/// # Arguments
///
/// * `workspace_path` - Path to workspace containing .devcontainer/Dockerfile
/// * `force_rebuild` - If true, rebuild image unconditionally
/// * `runtime_config` - Runtime configuration for output
///
/// # Returns
///
/// * `Ok(())` - Image is ready and up-to-date
/// * `Err` - Failed to build or check image
pub async fn ensure_image_built(
    workspace_path: &Path,
    force_rebuild: bool,
    runtime_config: &crate::cli::RuntimeConfig,
) -> Result<(), BundlerError> {
    // Fast pre-flight check to ensure Docker daemon is responsive
    check_docker_responsive().await?;

    let dockerfile_path = workspace_path.join(".devcontainer/Dockerfile");

    if !dockerfile_path.exists() {
        return Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "check_dockerfile".to_string(),
            reason: format!(
                "Dockerfile not found at: {}\n\
                 \n\
                 To use Docker for cross-platform builds, you need a Dockerfile.\n\
                 The expected location is:\n\
                 {}\n\
                 \n\
                 This Dockerfile provides a Linux container with:\n\
                 • Rust toolchain (matching rust-toolchain.toml)\n\
                 • Wine + .NET 4.0 (for building Windows .msi installers)\n\
                 • NSIS (for building .exe installers)\n\
                 • Tools for .deb, .rpm, and AppImage creation\n\
                 \n\
                 See example and setup guide:\n\
                 https://github.com/cyrup/kodegen/tree/main/.devcontainer",
                dockerfile_path.display(),
                dockerfile_path.display()
            ),
        }));
    }

    // Force rebuild if requested
    if force_rebuild {
        runtime_config.progress("Force rebuilding Docker image (--rebuild-image)...");
        return build_docker_image(workspace_path, runtime_config).await;
    }

    // Check if image exists
    let check_output = timeout(
        Duration::from_secs(10), // Image check should be fast
        Command::new("docker")
            .args(["images", "-q", BUILDER_IMAGE_NAME])
            .output(),
    )
    .await
    .map_err(|_| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker images".to_string(),
            reason: "Docker image check timed out after 10 seconds.\n\
                     \n\
                     This usually indicates:\n\
                     • Docker daemon is hung or crashed\n\
                     • Docker data directory is on slow/failed storage\n\
                     • System is under extreme load\n\
                     \n\
                     Quick fixes:\n\
                     1. Check Docker: docker ps\n\
                     2. Restart daemon: sudo systemctl restart docker\n\
                     3. Check disk: df -h /var/lib/docker\n\
                     4. Check logs: journalctl -u docker -n 50"
                .to_string(),
        })
    })?
    .map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker images".to_string(),
            reason: e.to_string(),
        })
    })?;

    let image_id = String::from_utf8_lossy(&check_output.stdout)
        .trim()
        .to_string();

    if !image_id.is_empty() && image_id.len() >= 12 {
        // Image exists - check if it's up-to-date
        runtime_config.verbose_println(&format!(
            "Found existing Docker image: {}",
            &image_id[..12.min(image_id.len())]
        ));

        match is_image_up_to_date(&image_id, &dockerfile_path, runtime_config).await {
            Ok(true) => {
                // Check if image is too old (older than 7 days)
                if let Ok(age_days) = get_image_age_days(&image_id).await
                    && age_days > 7
                {
                    runtime_config.warn(&format!(
                        "Docker image is {} days old - rebuilding to get base image updates",
                        age_days
                    ));
                    return build_docker_image(workspace_path, runtime_config).await;
                }

                runtime_config.verbose_println("Docker image is up-to-date");
                return Ok(());
            }
            Ok(false) => {
                runtime_config.warn(&format!(
                    "Docker image {} is outdated (Dockerfile modified since image creation)",
                    BUILDER_IMAGE_NAME
                ));
                runtime_config.progress("Rebuilding Docker image...");
                return build_docker_image(workspace_path, runtime_config).await;
            }
            Err(e) => {
                // If we can't determine staleness, be conservative and rebuild
                runtime_config.warn(&format!(
                    "Could not verify image freshness: {}\nRebuilding to be safe...",
                    e
                ));
                return build_docker_image(workspace_path, runtime_config).await;
            }
        }
    }

    // Image doesn't exist - build it
    runtime_config.progress(&format!(
        "Building {} Docker image (this may take a few minutes)...",
        BUILDER_IMAGE_NAME
    ));
    build_docker_image(workspace_path, runtime_config).await
}
