//! Command execution functions for bundler operations.
//!
//! This module provides devcontainer management for Docker-based builds.

// Submodules
mod devcontainer;

// Re-export public API
pub use devcontainer::copy_embedded_devcontainer;

use crate::bundler::{BundleBinary, Bundler, PackageSettings, PackageType, SettingsBuilder};
use crate::cli::args::{Args, RuntimeConfig};
use crate::cli::docker::bundler::ContainerBundler;
use crate::cli::docker::image::ensure_image_built;
use crate::cli::docker::limits::ContainerLimits;
use crate::error::{BundlerError, CliError, Result};
use crate::metadata::load_manifest;
use crate::source::RepositorySource;

/// Execute the bundle command with parsed arguments
///
/// This is the main entry point that connects CLI args to the bundler library.
///
/// ## Flow:
/// 1. Validate arguments
/// 2. Load Cargo.toml metadata from repo
/// 3. Build binary if needed (cargo build --release)
/// 4. Parse platform string to PackageType
/// 5. Create Settings via SettingsBuilder
/// 6. Create Bundler and call bundle()
/// 7. Output artifact paths to stdout (one per line)
/// 8. Return exit code 0 on success, 1 on error
pub async fn execute_command(args: Args, runtime_config: RuntimeConfig) -> Result<i32> {
    // Step 1: Validate arguments
    args.validate()
        .map_err(|e| BundlerError::Cli(CliError::InvalidArguments { reason: e }))?;

    runtime_config.verbose_println(&format!(
        "📦 Bundler starting for platform: {}",
        args.platform
    )).expect("Failed to write to stdout");

    // Step 2: Parse platform to determine build target
    let package_type = parse_platform_string(&args.platform)?;
    runtime_config.verbose_println(&format!("   Package type: {:?}", package_type)).expect("Failed to write to stdout");

    // Step 3: Check if Docker is needed BEFORE doing any work
    if needs_docker(&package_type) {
        runtime_config.verbose_println(&format!(
            "   Cross-platform build detected (current: {}, required: {})",
            std::env::consts::OS,
            required_os_for_package(&package_type)
        )).expect("Failed to write to stdout");
        runtime_config.verbose_println("   Using Docker container for bundling...").expect("Failed to write to stdout");

        // Ensure Docker image is built before attempting to use it
        ensure_image_built(false, &runtime_config).await?;

        // Pass the bundling task to Docker container
        // Container will clone, build, and bundle internally
        let limits = ContainerLimits::default();
        let container_bundler = ContainerBundler::new(
            args.source.clone(),
            args.output_binary.clone(),
            limits,
        );

        let artifact_path = container_bundler
            .bundle(package_type, &runtime_config)
            .await?;

        // Verify artifact exists at specified output path
        if !artifact_path.exists() {
            return Err(BundlerError::Cli(CliError::ExecutionFailed {
                command: "docker container bundle".to_string(),
                reason: format!(
                    "Container bundling completed but artifact not found at {}",
                    artifact_path.display()
                ),
            }));
        }

        runtime_config.success_println(&format!("✓ ✓ Artifact at: {}", artifact_path.display())).expect("Failed to write to stdout");
        println!("{}", artifact_path.display());
        return Ok(0);
    }

    // Step 4: Native platform execution - resolve source, build, and bundle
    let source = RepositorySource::parse(&args.source)?;
    let repo_path = source.resolve().await?;

    runtime_config.verbose_println(&format!("   Repository: {}", repo_path.display())).expect("Failed to write to stdout");

    // Step 5: Load Cargo.toml metadata
    let cargo_toml = repo_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(BundlerError::Cli(CliError::InvalidArguments {
            reason: format!("Cargo.toml not found at {}", cargo_toml.display()),
        }));
    }

    let manifest = load_manifest(&cargo_toml)?;
    runtime_config.verbose_println(&format!(
        "   Loaded manifest: {} v{}",
        manifest.metadata.name, manifest.metadata.version
    )).expect("Failed to write to stdout");
    runtime_config.verbose_println(&format!("   Binary: {}", manifest.binary_name)).expect("Failed to write to stdout");

    // Step 4: Determine cross-compilation target for NSIS on non-Windows
    let cross_compile_target = if package_type == PackageType::Exe && std::env::consts::OS != "windows" {
        Some("x86_64-pc-windows-gnu")
    } else {
        None
    };

    // Step 5: Build binary
    runtime_config.section("🔨 Building binary...").expect("Failed to write to stdout");

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("--bin")
        .arg(&manifest.binary_name);

    // Add cross-compilation target if needed
    if let Some(target) = cross_compile_target {
        runtime_config.verbose_println(&format!("   Cross-compiling for {}", target)).expect("Failed to write to stdout");
        cmd.arg("--target").arg(target);
    }

    // Pipe stdout and stderr to capture output
    let mut child = cmd
        .current_dir(&repo_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "cargo build".to_string(),
                reason: e.to_string(),
            })
        })?;

    // Stream both stdout and stderr concurrently through OutputManager
    tokio::join!(
        async {
            if let Some(stdout) = child.stdout.take() {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    runtime_config.indent(&line).expect("Failed to write cargo output");
                }
            }
        },
        async {
            if let Some(stderr) = child.stderr.take() {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    runtime_config.indent(&line).expect("Failed to write cargo output");
                }
            }
        }
    );

    // Wait for build to complete
    let build_status = child.wait().await.map_err(|e| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "cargo build".to_string(),
            reason: e.to_string(),
        })
    })?;

    if !build_status.success() {
        return Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "cargo build".to_string(),
            reason: format!("Build failed with exit code: {:?}", build_status.code()),
        }));
    }

    runtime_config.verbose_println("   ✓ Build completed").expect("Failed to write to stdout");

    // Step 6: Determine binary path
    let target_dir = if let Some(target) = cross_compile_target {
        // Cross-compilation (e.g., NSIS builds for Windows on macOS)
        repo_path.join("target").join(target).join("release")
    } else {
        // Default native macOS build
        repo_path.join("target").join("release")
    };
    
    // Windows binaries have .exe extension
    let binary_name_with_ext = if cross_compile_target.is_some() {
        format!("{}.exe", manifest.binary_name)
    } else {
        manifest.binary_name.clone()
    };
    let binary_path = target_dir.join(&binary_name_with_ext);

    runtime_config.verbose_println(&format!("   Expected binary path: {}", binary_path.display())).expect("Failed to write to stdout");

    if !binary_path.exists() {
        // List what files ARE in the target directory for debugging
        let mut available_files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    available_files.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }

        return Err(BundlerError::Cli(CliError::InvalidArguments {
            reason: format!(
                "Binary not found at {}\n\
                 \n\
                 Expected: {}\n\
                 Available files in {}: {:?}\n\
                 \n\
                 Did cargo build create the binary with a different name?",
                binary_path.display(),
                manifest.binary_name,
                target_dir.display(),
                available_files
            ),
        }));
    }

    let binary_metadata = std::fs::metadata(&binary_path)?;
    runtime_config.verbose_println(&format!(
        "   ✓ Binary found: {} ({} bytes)",
        binary_path.display(),
        binary_metadata.len()
    )).expect("Failed to write to stdout");

    // Step 6: Create PackageSettings from metadata
    let package_settings = PackageSettings {
        product_name: manifest.metadata.name.clone(),
        version: manifest.metadata.version.clone(),
        description: manifest.metadata.description.clone(),
        homepage: manifest.metadata.homepage.clone(),
        authors: Some(manifest.metadata.authors.clone()),
        default_run: Some(manifest.binary_name.clone()),
    };

    // Step 7: Create BundleBinary
    let bundle_binary = BundleBinary::new(manifest.binary_name.clone(), true);

    // Step 8: Build Settings via SettingsBuilder
    let settings = SettingsBuilder::new()
        .project_out_directory(&target_dir)
        .package_settings(package_settings)
        .bundle_settings(manifest.bundle_settings)
        .binaries(vec![bundle_binary])
        .package_types(vec![package_type])
        .build()?;

    runtime_config.section(&format!(
        "📦 Creating {} package...",
        platform_display_name(&package_type)
    )).expect("Failed to write to stdout");

    // Step 9: Bundle - native platform only (Docker handled earlier)
    runtime_config.verbose_println("   Native platform build").expect("Failed to write to stdout");
    let bundler = Bundler::new(settings).await?;
    let artifacts = bundler.bundle().await?;

    // Extract paths from artifacts
    let artifact_paths: Vec<std::path::PathBuf> = artifacts.into_iter().flat_map(|a| a.paths).collect();

    // Step 10: Handle output
    if artifact_paths.is_empty() {
        runtime_config.warning_println("⚠️  No artifacts created").expect("Failed to write to stdout");
        return Ok(1);
    }

    runtime_config.success_println(&format!("✓ Created {} artifact(s)", artifact_paths.len())).expect("Failed to write to stdout");

    // Step 11: Move artifact to specified output path
    let output_path = &args.output_binary;

    // Get the main artifact path (first path)
    let source_path = artifact_paths.first().ok_or_else(|| {
        BundlerError::Cli(CliError::ExecutionFailed {
            command: "get artifact path".to_string(),
            reason: "No artifact paths returned from bundler".to_string(),
        })
    })?;

    runtime_config.verbose_println(&format!(
        "   Moving artifact:\n      from: {}\n      to:   {}",
        source_path.display(),
        output_path.display()
    )).expect("Failed to write to stdout");

    // Bundler responsibility: create parent directories
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "create output directory".to_string(),
                reason: format!("Failed to create {}: {}", parent.display(), e),
            })
        })?;
    }

    // Move artifact to specified output path (handles cross-filesystem moves)
    tokio::fs::copy(source_path, output_path)
        .await
        .map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "copy artifact".to_string(),
                reason: format!(
                    "Failed to copy artifact from {} to {}: {}",
                    source_path.display(),
                    output_path.display(),
                    e
                ),
            })
        })?;

    // Remove source file after successful copy
    tokio::fs::remove_file(source_path)
        .await
        .map_err(|e| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "remove source artifact".to_string(),
                reason: format!(
                    "Failed to remove source artifact {}: {}",
                    source_path.display(),
                    e
                ),
            })
        })?;

    // Contract enforcement: verify file exists at destination
    if !output_path.exists() {
        return Err(BundlerError::Cli(CliError::ExecutionFailed {
            command: "verify output".to_string(),
            reason: format!(
                "Move reported success but file does not exist at {}",
                output_path.display()
            ),
        }));
    }

    runtime_config.success_println(&format!("✓ Artifact at: {}", output_path.display())).expect("Failed to write to stdout");

    // Output the final path to stdout (for diagnostics)
    println!("{}", output_path.display());

    Ok(0)
}

/// Parse platform string to PackageType enum
fn parse_platform_string(platform: &str) -> Result<PackageType> {
    match platform.to_lowercase().as_str() {
        "deb" => Ok(PackageType::Deb),
        "rpm" => Ok(PackageType::Rpm),
        "appimage" => Ok(PackageType::AppImage),
        "dmg" => Ok(PackageType::Dmg),
        "exe" => Ok(PackageType::Exe),
        _ => Err(BundlerError::Cli(CliError::InvalidArguments {
            reason: format!(
                "Unsupported platform '{}'. Valid: deb, rpm, appimage, dmg, nsis",
                platform
            ),
        })),
    }
}

/// Get human-readable platform name
fn platform_display_name(package_type: &PackageType) -> &'static str {
    match package_type {
        PackageType::Deb => "Debian Package (.deb)",
        PackageType::Rpm => "RedHat Package (.rpm)",
        PackageType::AppImage => "Linux AppImage",
        PackageType::Dmg => "macOS Disk Image (.dmg)",
        PackageType::MacOsBundle => "macOS Application Bundle (.app)",
        PackageType::Exe => "Windows NSIS Installer (.exe)",
    }
}

/// Determine which host OS is required for a package type
fn required_os_for_package(package_type: &PackageType) -> &'static str {
    match package_type {
        PackageType::Deb | PackageType::Rpm | PackageType::AppImage => "linux",
        PackageType::Dmg | PackageType::MacOsBundle => "macos",
        PackageType::Exe => "windows",
    }
}

/// Check if Docker is needed for cross-platform bundling
///
/// Returns false if:
/// - Already running inside Docker (detected via /.dockerenv, cgroup, or env var)
/// - Package type matches current OS (native build)
///
/// Returns true if:
/// - Running on host OS and package requires different OS (cross-platform build)
fn needs_docker(package_type: &PackageType) -> bool {
    // Auto-detect if we're already inside a Docker container
    // If so, use native tools (container has all required tooling installed)
    let in_docker = {
        // Check 1: /.dockerenv file exists (standard Docker indicator)
        if std::path::Path::new("/.dockerenv").exists() {
            true
        }
        // Check 2: /proc/1/cgroup contains "docker" or "buildkit"
        else if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
            cgroup.contains("docker") || cgroup.contains("buildkit")
        }
        // Check 3: Explicit environment variable
        else {
            std::env::var("KODEGEN_IN_DOCKER").is_ok()
        }
    };

    if in_docker {
        // Inside Docker: use native tools (deb, rpm, nsis all work natively)
        return false;
    }

    // On host system: use Docker for cross-platform builds
    let required_os = required_os_for_package(package_type);
    let current_os = std::env::consts::OS;
    required_os != current_os
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_platform_string() {
        assert!(matches!(
            parse_platform_string("deb").unwrap(),
            PackageType::Deb
        ));
        assert!(matches!(
            parse_platform_string("DMG").unwrap(),
            PackageType::Dmg
        ));
        assert!(matches!(
            parse_platform_string("macos-bundle").unwrap(),
            PackageType::MacOsBundle
        ));
        assert!(matches!(
            parse_platform_string("app").unwrap(),
            PackageType::MacOsBundle
        ));
        assert!(matches!(
            parse_platform_string("exe").unwrap(),
            PackageType::Exe
        ));
        assert!(matches!(
            parse_platform_string("exe").unwrap(),
            PackageType::Exe
        ));
        assert!(parse_platform_string("invalid").is_err());
    }
}
