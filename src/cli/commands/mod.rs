//! Command execution functions for bundler operations.
//!
//! This module provides devcontainer management for Docker-based builds.

// Submodules
mod devcontainer;

use crate::bundler::{BundleBinary, Bundler, PackageSettings, PackageType, SettingsBuilder};
use crate::cli::args::{Args, RuntimeConfig};
use crate::error::{BundlerError, CliError, Result};
use crate::metadata::load_manifest;

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
    ));
    runtime_config.verbose_println(&format!("   Repository: {}", args.repo_path.display()));
    runtime_config.verbose_println(&format!("   Binary: {}", args.binary_name));
    runtime_config.verbose_println(&format!("   Version: {}", args.version));

    // Step 2: Load Cargo.toml metadata
    let cargo_toml = args.repo_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(BundlerError::Cli(CliError::InvalidArguments {
            reason: format!("Cargo.toml not found at {}", cargo_toml.display()),
        }));
    }

    let manifest = load_manifest(&cargo_toml)?;
    runtime_config.verbose_println(&format!(
        "   Loaded manifest: {} v{}",
        manifest.metadata.name, manifest.metadata.version
    ));

    // Step 3: Build binary if needed
    if !args.no_build {
        runtime_config.section("🔨 Building binary...");

        let build_status = std::process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--bin")
            .arg(&args.binary_name)
            .current_dir(&args.repo_path)
            .status()
            .map_err(|e| {
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

        runtime_config.verbose_println("   ✓ Build completed");
    } else {
        runtime_config.verbose_println("   Skipping build (--no-build specified)");
    }

    // Step 4: Parse platform string to PackageType
    let package_type = parse_platform_string(&args.platform)?;
    runtime_config.verbose_println(&format!("   Package type: {:?}", package_type));

    // Step 5: Determine binary path
    let target_dir = args.repo_path.join("target").join("release");
    let binary_path = target_dir.join(&args.binary_name);

    if !binary_path.exists() {
        return Err(BundlerError::Cli(CliError::InvalidArguments {
            reason: format!(
                "Binary not found at {}. Did you forget to build?",
                binary_path.display()
            ),
        }));
    }

    runtime_config.verbose_println(&format!("   Binary path: {}", binary_path.display()));

    // Step 6: Create PackageSettings from metadata
    let package_settings = PackageSettings {
        product_name: manifest.metadata.name.clone(),
        version: args.version.clone(),
        description: manifest.metadata.description.clone(),
        homepage: manifest.metadata.homepage.clone(),
        authors: Some(manifest.metadata.authors.clone()),
        default_run: Some(args.binary_name.clone()),
    };

    // Step 7: Create BundleBinary
    let bundle_binary = BundleBinary::new(binary_path.to_string_lossy().to_string(), true)
        .set_src_path(Some(binary_path.to_string_lossy().to_string()));

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
    ));

    // Step 9: Create Bundler and execute
    let bundler = Bundler::new(settings).await?;
    let artifacts = bundler.bundle().await?;

    // Step 10: Handle output
    if artifacts.is_empty() {
        runtime_config.warning_println("⚠️  No artifacts created");
        return Ok(1);
    }

    runtime_config.success_println(&format!("✓ Created {} artifact(s)", artifacts.len()));

    // Step 11: Handle --output-binary if specified
    if let Some(output_path) = &args.output_binary {
        // Get the main artifact path (first path of first artifact)
        let source_path = artifacts[0].paths.first().ok_or_else(|| {
            BundlerError::Cli(CliError::ExecutionFailed {
                command: "get artifact path".to_string(),
                reason: "No artifact paths returned from bundler".to_string(),
            })
        })?;

        runtime_config.verbose_println(&format!(
            "   Moving artifact:\n      from: {}\n      to:   {}",
            source_path.display(),
            output_path.display()
        ));

        // Bundler responsibility: create parent directories
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "create output directory".to_string(),
                    reason: format!("Failed to create {}: {}", parent.display(), e),
                })
            })?;
        }

        // Move artifact to specified output path (simple rename, same filesystem)
        tokio::fs::rename(source_path, output_path)
            .await
            .map_err(|e| {
                BundlerError::Cli(CliError::ExecutionFailed {
                    command: "move artifact".to_string(),
                    reason: format!(
                        "Failed to move artifact from {} to {}: {}",
                        source_path.display(),
                        output_path.display(),
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

        runtime_config.success_println(&format!("✓ Artifact at: {}", output_path.display()));

        // Output the final path to stdout (for diagnostics)
        println!("{}", output_path.display());
    } else {
        // Legacy behavior: output artifact paths in their original locations
        for artifact in &artifacts {
            for path in &artifact.paths {
                println!("{}", path.display());
                runtime_config.verbose_println(&format!("   Artifact: {}", path.display()));
            }
        }
    }

    Ok(0)
}

/// Parse platform string to PackageType enum
fn parse_platform_string(platform: &str) -> Result<PackageType> {
    match platform.to_lowercase().as_str() {
        "deb" => Ok(PackageType::Deb),
        "rpm" => Ok(PackageType::Rpm),
        "appimage" => Ok(PackageType::AppImage),
        "dmg" => Ok(PackageType::Dmg),
        "macos-bundle" | "app" | "bundle" => Ok(PackageType::MacOsBundle),
        "nsis" | "exe" => Ok(PackageType::Nsis),
        _ => Err(BundlerError::Cli(CliError::InvalidArguments {
            reason: format!(
                "Unsupported platform '{}'. Valid: deb, rpm, appimage, dmg, macos-bundle (app), nsis (exe)",
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
        PackageType::Nsis => "Windows NSIS Installer (.exe)",
    }
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
            parse_platform_string("nsis").unwrap(),
            PackageType::Nsis
        ));
        assert!(matches!(
            parse_platform_string("exe").unwrap(),
            PackageType::Nsis
        ));
        assert!(parse_platform_string("invalid").is_err());
    }
}
