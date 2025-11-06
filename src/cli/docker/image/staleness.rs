//! Docker image staleness checking and age calculations.

use crate::error::{BundlerError, CliError};
use chrono::{DateTime, Utc};
use std::path::Path;
use tokio::process::Command;

use super::utils::humanize_duration;

/// Tolerance window for timestamp comparison to handle filesystem precision mismatches.
///
/// Filesystems have varying timestamp precision:
/// - HFS+: 1 second
/// - APFS/ext4/XFS: 1 nanosecond  
/// - NTFS: 100 nanoseconds
///
/// A 2-second tolerance prevents false positives from:
/// - Filesystem timestamp rounding
/// - Clock jitter
/// - Build process startup time
const STALENESS_TOLERANCE_SECS: i64 = 2;

/// Checks if Docker image is up-to-date with current Dockerfile.
///
/// Compares Dockerfile modification time against Docker image creation time.
///
/// # Arguments
///
/// * `image_id` - Docker image ID or tag
/// * `dockerfile_path` - Path to Dockerfile
/// * `runtime_config` - Runtime config for verbose output
///
/// # Returns
///
/// * `Ok(true)` - Image is up-to-date (created after last Dockerfile modification)
/// * `Ok(false)` - Image is stale (Dockerfile modified after image creation)
/// * `Err` - Could not determine staleness
pub async fn is_image_up_to_date(
    image_id: &str,
    dockerfile_path: &Path,
    runtime_config: &crate::cli::RuntimeConfig,
) -> Result<bool, BundlerError> {
    // Get image creation timestamp from Docker
    let inspect_output = Command::new("docker")
        .args(["inspect", "-f", "{{.Created}}", image_id])
        .output()
        .await
        .map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: format!("docker inspect {}", image_id),
                reason: e.to_string(),
            })
        })?;

    if !inspect_output.status.success() {
        let stderr = String::from_utf8_lossy(&inspect_output.stderr);
        return Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker inspect".to_string(),
            reason: format!("Failed to inspect image: {}", stderr),
        }));
    }

    let image_created_str = String::from_utf8_lossy(&inspect_output.stdout)
        .trim()
        .to_string();

    // Parse Docker's RFC3339 timestamp
    let image_created_time = DateTime::parse_from_rfc3339(&image_created_str).map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "parse_timestamp".to_string(),
            reason: format!(
                "Invalid timestamp from Docker '{}': {}",
                image_created_str, e
            ),
        })
    })?;

    // Get Dockerfile modification time
    let dockerfile_metadata = std::fs::metadata(dockerfile_path).map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "stat_dockerfile".to_string(),
            reason: format!("Cannot read Dockerfile metadata: {}", e),
        })
    })?;

    let dockerfile_modified = dockerfile_metadata.modified().map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "get_mtime".to_string(),
            reason: format!("Cannot get Dockerfile modification time: {}", e),
        })
    })?;

    let dockerfile_time: DateTime<Utc> = dockerfile_modified.into();
    let image_time: DateTime<Utc> = image_created_time.into();

    // Compare timestamps with tolerance for filesystem precision
    let time_diff_secs = (dockerfile_time - image_time).num_seconds();

    if time_diff_secs > STALENESS_TOLERANCE_SECS {
        // Dockerfile modified significantly after image creation - definitely stale
        runtime_config.verbose_println(&format!(
            "Dockerfile modified {} seconds after image creation (tolerance: {}s)",
            time_diff_secs,
            STALENESS_TOLERANCE_SECS
        ));
        runtime_config.verbose_println(&format!(
            "  Dockerfile: {} | Image: {}",
            dockerfile_time.format("%Y-%m-%d %H:%M:%S UTC"),
            image_time.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        Ok(false) // Definitely stale
    } else if time_diff_secs < -STALENESS_TOLERANCE_SECS {
        // Image created significantly after Dockerfile - definitely fresh
        runtime_config.verbose_println(&format!(
            "Image is up-to-date (created {} after Dockerfile)",
            humanize_duration(-time_diff_secs)
        ));
        Ok(true) // Definitely fresh
    } else {
        // Within tolerance window - treat as fresh to avoid false positives
        runtime_config.verbose_println(&format!(
            "Image and Dockerfile times very close ({}s difference, tolerance: {}s) - treating as fresh",
            time_diff_secs.abs(),
            STALENESS_TOLERANCE_SECS
        ));
        Ok(true) // Within tolerance - assume fresh
    }
}

/// Gets the age of a Docker image in days.
///
/// # Arguments
///
/// * `image_id` - Docker image ID or tag
///
/// # Returns
///
/// * `Ok(days)` - Number of days since image was created (always >= 0)
/// * `Err` - Could not determine image age
///
/// # Clock Skew Handling
///
/// If the image timestamp is in the future (due to clock synchronization issues),
/// this function logs a warning and returns 0 (treats image as brand new).
/// This prevents negative age values from bypassing rebuild checks.
pub async fn get_image_age_days(image_id: &str) -> Result<u64, BundlerError> {
    // Get image creation timestamp from Docker
    let inspect_output = Command::new("docker")
        .args(["inspect", "-f", "{{.Created}}", image_id])
        .output()
        .await
        .map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: format!("docker inspect {}", image_id),
                reason: e.to_string(),
            })
        })?;

    if !inspect_output.status.success() {
        let stderr = String::from_utf8_lossy(&inspect_output.stderr);
        return Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "docker inspect".to_string(),
            reason: format!("Failed to get image creation time: {}", stderr),
        }));
    }

    let created_str = String::from_utf8_lossy(&inspect_output.stdout)
        .trim()
        .to_string();

    // Parse Docker's RFC3339 timestamp
    let created_time = DateTime::parse_from_rfc3339(&created_str).map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "parse_timestamp".to_string(),
            reason: format!("Invalid timestamp '{}': {}", created_str, e),
        })
    })?;

    let now = Utc::now();
    let created_utc: DateTime<Utc> = created_time.into();

    // Detect clock skew: image timestamp is in the future
    if created_utc > now {
        log::warn!(
            "Docker image timestamp ({}) is in the future (current time: {}). \
             This indicates system clock is incorrect or out of sync. \
             Treating image as brand new (age 0 days) to avoid rebuild errors.",
            created_utc.format("%Y-%m-%d %H:%M:%S UTC"),
            now.format("%Y-%m-%d %H:%M:%S UTC")
        );
        return Ok(0);
    }

    Ok((now - created_utc).num_days() as u64)
}
