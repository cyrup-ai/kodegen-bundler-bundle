//! Out-of-memory detection and error reporting for Docker containers.

use crate::bundler::PackageType;
use crate::error::{BundlerError, CliError};
use tokio::process::Command;

/// Out-of-memory detector for Docker containers.
pub struct OomDetector {
    memory_limit: String,
    memory_swap: String,
}

impl OomDetector {
    /// Creates a new OOM detector.
    ///
    /// # Arguments
    ///
    /// * `memory_limit` - Current memory limit (e.g., "4g")
    /// * `memory_swap` - Current memory+swap limit (e.g., "8g")
    pub fn new(memory_limit: String, memory_swap: String) -> Self {
        Self {
            memory_limit,
            memory_swap,
        }
    }

    /// Check if container was killed by OOM via Docker inspect API.
    ///
    /// # Arguments
    ///
    /// * `container_name` - Name of the container to check
    ///
    /// # Returns
    ///
    /// `true` if container was OOM killed, `false` otherwise
    pub async fn check_container_oom_status(container_name: &str) -> Result<bool, std::io::Error> {
        let output = Command::new("docker")
            .args([
                "inspect",
                container_name,
                "--format",
                "{{.State.OOMKilled}}",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(false); // Container doesn't exist or inspect failed
        }

        let oom_killed = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();

        Ok(oom_killed == "true")
    }

    /// Detects if process failure was due to OOM.
    ///
    /// Checks multiple indicators:
    /// - Exit code 137 (SIGKILL from OOM)
    /// - Stderr strings containing OOM indicators
    /// - Docker container OOMKilled status
    ///
    /// # Arguments
    ///
    /// * `exit_code` - Process exit code
    /// * `stderr_lines` - Captured stderr lines
    /// * `container_name` - Container name for Docker API check
    ///
    /// # Returns
    ///
    /// `true` if OOM detected, `false` otherwise
    pub async fn is_oom_failure(
        &self,
        exit_code: i32,
        stderr_lines: &[String],
        container_name: &str,
    ) -> bool {
        // Check exit code 137 (SIGKILL from OOM)
        let is_oom_exit_code = exit_code == 137;

        // Check stderr strings for OOM indicators
        let stderr_str = stderr_lines.join("\n");
        let is_oom_stderr = stderr_str.contains("OOMKilled")
            || stderr_str.contains("out of memory")
            || stderr_str.contains("Out of memory")
            || stderr_str.contains("OutOfMemoryError")
            || stderr_str.contains("Cannot allocate memory")
            || stderr_str.to_lowercase().contains("oom");

        // Check Docker container status (most reliable method)
        let is_oom_status = Self::check_container_oom_status(container_name)
            .await
            .unwrap_or(false);

        is_oom_exit_code || is_oom_stderr || is_oom_status
    }

    /// Formats an OOM error with helpful diagnostic information.
    ///
    /// # Arguments
    ///
    /// * `platform` - Platform being bundled
    /// * `stderr_lines` - Captured stderr for inclusion in error
    /// * `exit_code` - Process exit code for detection method
    /// * `container_name` - Container name (for checking if status-based detection)
    ///
    /// # Returns
    ///
    /// Formatted error with suggestions
    pub async fn format_oom_error(
        &self,
        platform: PackageType,
        stderr_lines: &[String],
        exit_code: i32,
        container_name: &str,
    ) -> BundlerError {
        let platform_str = super::platform::platform_type_to_string(platform);

        // Get system memory info
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let total_memory_gb = sys.total_memory() / 1024 / 1024 / 1024;

        let mut reason = String::from("Container ran out of memory during build.\n\n");

        // Add detection method for debugging
        let is_oom_status = Self::check_container_oom_status(container_name)
            .await
            .unwrap_or(false);

        if is_oom_status {
            reason.push_str("(Detected via Docker container status)\n");
        } else if exit_code == 137 {
            reason.push_str("(Detected via exit code 137 - SIGKILL)\n");
        } else {
            reason.push_str("(Detected via error message)\n");
        }

        reason.push_str(&format!(
            "\nCurrent memory limit: {} (swap: {})\n\
             \n\
             The container exhausted available memory while building. This typically happens when:\n\
             • Building large Rust projects with many dependencies\n\
             • Parallel compilation uses more RAM than available\n\
             • Debug builds require more memory than release builds\n\
             \n\
             Solutions:\n\
             1. Increase memory limit:\n\
                cargo run -p kodegen_bundler_release -- bundle --platform {} --docker-memory 8g\n\
             \n\
             2. Build fewer platforms in parallel (run multiple times with --platform)\n\
             \n\
             3. Use release builds (they use less memory):\n\
                cargo run -p kodegen_bundler_release -- bundle --platform {} --release\n\
             \n\
             4. Check available system memory: {} GB total",
            self.memory_limit,
            self.memory_swap,
            platform_str,
            platform_str,
            total_memory_gb,
        ));

        // Include actual stderr so user can see the real error
        let stderr_str = stderr_lines.join("\n");
        if !stderr_str.is_empty() {
            reason.push_str("\n\n=== ACTUAL STDERR OUTPUT ===\n");
            reason.push_str(&stderr_str);
        }

        BundlerError::Cli(CliError::ExecutionFailed {
            command: format!("bundle {} in container", platform_str),
            reason,
        })
    }

    /// Formats a generic container failure error.
    ///
    /// # Arguments
    ///
    /// * `platform` - Platform being bundled
    /// * `exit_code` - Process exit code
    /// * `stderr_lines` - Captured stderr output
    ///
    /// # Returns
    ///
    /// Formatted error with captured output
    pub fn format_generic_error(
        &self,
        platform: PackageType,
        exit_code: i32,
        stderr_lines: &[String],
    ) -> BundlerError {
        let platform_str = super::platform::platform_type_to_string(platform);
        let stderr_str = stderr_lines.join("\n");

        let error_output = if !stderr_str.is_empty() {
            format!("stderr:\n{}", stderr_str)
        } else {
            "No error output captured".to_string()
        };

        BundlerError::Cli(CliError::ExecutionFailed {
            command: format!("bundle {} in container", platform_str),
            reason: format!(
                "Container bundling failed with exit code: {}\n\n{}",
                exit_code, error_output
            ),
        })
    }
}
