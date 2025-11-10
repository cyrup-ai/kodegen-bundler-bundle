//! Docker image configuration and constants.

use std::time::Duration;

/// Docker image name for the release builder container
pub const BUILDER_IMAGE_NAME: &str = "kodegen-release-builder";

/// Timeout for Docker info check (5 seconds)
/// Quick daemon availability check shouldn't take long
pub const DOCKER_INFO_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for Docker image build operations (30 minutes)
/// Image builds can take a long time due to base image downloads, apt updates, etc.
pub const DOCKER_BUILD_TIMEOUT: Duration = Duration::from_secs(1800);

/// Platform-specific Docker startup instructions
#[cfg(target_os = "macos")]
pub const DOCKER_START_HELP: &str = "Start Docker Desktop from Applications or Spotlight";

#[cfg(target_os = "linux")]
pub const DOCKER_START_HELP: &str = "Start Docker daemon: sudo systemctl start docker";
