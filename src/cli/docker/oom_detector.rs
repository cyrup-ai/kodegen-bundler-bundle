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
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Distinguish "not found" from other errors
            if stderr.contains("No such container") || stderr.contains("No such object") {
                log::debug!(
                    "Container {} already removed (possibly OOM-killed with --rm)",
                    container_name
                );
                return Ok(false);
            }

            log::warn!(
                "Docker inspect failed for {}: {}",
                container_name,
                stderr
            );
            return Ok(false);
        }

        let oom_killed = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();

        Ok(oom_killed == "true")
    }

    /// Detects if process failure was due to OOM.
    ///
    /// Uses priority-based detection to avoid race conditions:
    /// 1. Check stderr first (most reliable, always available)
    /// 2. Check Docker container OOMKilled status (requires container exists)
    /// 3. Check exit code 137 as final hint (least reliable)
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
        // Priority 1: Check stderr first (most reliable, always available)
        let stderr_str = stderr_lines.join("\n");
        let is_oom_stderr = stderr_str.contains("OOMKilled")
            || stderr_str.contains("out of memory")
            || stderr_str.contains("Out of memory")
            || stderr_str.contains("OutOfMemoryError")
            || stderr_str.contains("Cannot allocate memory")
            || stderr_str.to_lowercase().contains("oom");

        if is_oom_stderr {
            return true; // High confidence
        }

        // Priority 2: Check Docker status (container should exist since we removed --rm)
        let is_oom_status = Self::check_container_oom_status(container_name)
            .await
            .unwrap_or(false);

        if is_oom_status {
            return true; // Confirmed by Docker
        }

        // Priority 3: Exit code 137 alone is NOT sufficient evidence
        // Only return true if we have supporting indicators
        if exit_code == 137 {
            // Check for explicit non-OOM kill indicators in stderr
            let stderr_lower = stderr_str.to_lowercase();
            
            if stderr_lower.contains("killed by user")
                || stderr_lower.contains("sigkill") 
                || stderr_lower.contains("shutdown")
                || stderr_lower.contains("terminated by signal")
            {
                // Explicitly killed for non-OOM reason
                return false;
            }
            
            // If stderr is completely empty with exit 137, be conservative
            if stderr_str.trim().is_empty() {
                log::warn!(
                    "Container exited with 137 (SIGKILL) but no stderr output. \
                     Could be OOM, user kill, or system shutdown. Not diagnosing as OOM \
                     without stronger evidence."
                );
                return false;
            }
            
            // If we have stderr but no clear OOM or non-OOM markers,
            // be conservative - don't assume OOM
            log::debug!(
                "Container killed with SIGKILL (137), no clear OOM evidence. \
                 First stderr lines: {}",
                stderr_str.lines().take(3).collect::<Vec<_>>().join("; ")
            );
            return false;
        }

        false
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

    /// Formats an error for SIGKILL (137) without OOM evidence.
    ///
    /// # Arguments
    ///
    /// * `platform` - Platform being bundled
    /// * `stderr_lines` - Captured stderr output
    ///
    /// # Returns
    ///
    /// Formatted error explaining possible SIGKILL causes
    pub fn format_sigkill_error(
        &self,
        platform: PackageType,
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
                "Container was killed with SIGKILL (exit code 137).\n\
                 \n\
                 This can indicate:\n\
                 • Out of memory (but no OOM markers found in logs/status)\n\
                 • User cancellation (Ctrl+C, docker kill, kill -9)\n\
                 • System shutdown or reboot\n\
                 • PID limit reached (--pids-limit)\n\
                 • Docker daemon crash or restart\n\
                 \n\
                 To diagnose:\n\
                 • Check system logs: journalctl -u docker (Linux) or Console.app (macOS)\n\
                 • Check kernel logs: dmesg | grep -i oom\n\
                 • Verify container limits: --docker-memory, --cpus, --pids-limit\n\
                 • Check if you manually killed the process\n\
                 \n\
                 {}",
                error_output
            ),
        })
    }
}
