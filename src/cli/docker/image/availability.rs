//! Docker daemon availability checking.

use crate::error::{BundlerError, CliError};
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::timeout;

use super::config::{DOCKER_INFO_TIMEOUT, DOCKER_START_HELP};

/// Checks if Docker is installed and the daemon is running.
///
/// # Returns
///
/// * `Ok(())` - Docker is available
/// * `Err` - Docker is not installed or daemon is not running
pub async fn check_docker_available() -> Result<(), BundlerError> {
    let status_result = timeout(
        DOCKER_INFO_TIMEOUT,
        Command::new("docker")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    )
    .await;

    match status_result {
        // Timeout occurred
        Err(_) => Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker info".to_string(),
            reason: format!(
                "Docker daemon check timed out after {} seconds.\n\
                     \n\
                     This usually means Docker is not responding.\n\
                     {}\n\
                     \n\
                     If Docker is running, check: docker ps",
                DOCKER_INFO_TIMEOUT.as_secs(),
                DOCKER_START_HELP
            ),
        })),

        // Command succeeded
        Ok(Ok(status)) if status.success() => Ok(()),

        // Docker command exists but daemon isn't responding
        Ok(Ok(status)) => {
            let exit_code = status.code().unwrap_or(-1);
            Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "docker info".to_string(),
                reason: format!(
                    "Docker daemon is not responding (exit code: {}).\n\
                     \n\
                     {} \n\
                     \n\
                     If Docker is installed, ensure the daemon is running.\n\
                     If not installed, visit: https://docs.docker.com/get-docker/",
                    exit_code, DOCKER_START_HELP
                ),
            }))
        }

        // Docker command not found - not installed
        Ok(Err(e)) => Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker".to_string(),
            reason: format!(
                "Docker command not found: {}\n\
                     \n\
                     Docker does not appear to be installed.\n\
                     Install from: https://docs.docker.com/get-docker/\n\
                     \n\
                     Platform-specific instructions:\n\
                     • macOS: Install Docker Desktop (includes GUI and CLI)\n\
                     • Linux: Install docker.io (Ubuntu/Debian) or docker-ce (others)\n\
                     • Windows: Install Docker Desktop",
                e
            ),
        })),
    }
}
