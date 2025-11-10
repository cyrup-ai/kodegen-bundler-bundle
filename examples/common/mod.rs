//! Shared utilities for platform bundler examples

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub type LogFile = Arc<Mutex<File>>;

/// Write to both console and log file with flush for real-time output
pub fn log(log_file: &LogFile, msg: &str) {
    println!("{}", msg);
    if let Ok(mut file) = log_file.lock() {
        let _ = writeln!(file, "{}", msg);
        let _ = file.flush();
    }
}

/// Clean up previous artifacts and logs for this platform
pub fn cleanup(platform: &str, package_id: &str) {
    // Remove previous log file
    let log_path = format!("./tmp/test_{}.log", platform);
    let _ = std::fs::remove_file(&log_path);

    // Remove previous bundle artifact
    let extension = platform_extension(platform);
    let bundle_path = format!("./tmp/bundle/{}.{}", package_id, extension);
    let _ = std::fs::remove_file(&bundle_path);
}

/// Create log file in ./tmp directory (after cleanup)
pub fn setup_log_file(platform: &str) -> Result<LogFile, std::io::Error> {
    std::fs::create_dir_all("./tmp")?;
    let log_file = File::create(format!("./tmp/test_{}.log", platform))?;
    Ok(Arc::new(Mutex::new(log_file)))
}

/// Map platform name to file extension
pub fn platform_extension(platform: &str) -> &'static str {
    match platform {
        "deb" => "deb",
        "rpm" => "rpm",
        "appimage" => "AppImage",
        "dmg" => "dmg",
        "exe" => "exe",
        _ => panic!("Unsupported platform: {}", platform),
    }
}

/// Construct output file path in ./tmp/bundle directory
pub fn construct_output_path(package_id: &str, extension: &str) -> Result<PathBuf, std::io::Error> {
    // Create ./tmp/bundle directory
    std::fs::create_dir_all("./tmp/bundle")?;

    Ok(PathBuf::from(format!("./tmp/bundle/{}.{}", package_id, extension)))
}

/// Spawn bundler subprocess
pub fn spawn_bundler(
    platform: &str,
    output_file: &PathBuf,
    source: &str,
) -> Result<Child, std::io::Error> {
    Command::new("cargo")
        .args(["run", "--release", "--"])
        .arg("--source")
        .arg(source)
        .arg("--platform")
        .arg(platform)
        .arg("--output-binary")
        .arg(output_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Stream stdout and stderr to console and log file concurrently
pub fn stream_output(child: &mut Child, log_file: &LogFile) {
    // Spawn thread to read stdout concurrently
    let stdout_log = Arc::clone(log_file);
    let stdout_handle = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("    {}", line);
                    if let Ok(mut file) = stdout_log.lock() {
                        let _ = writeln!(file, "{}", line);
                        let _ = file.flush();
                    }
                }
            }
        })
    });

    // Spawn thread to read stderr concurrently
    let stderr_log = Arc::clone(log_file);
    let stderr_handle = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("    {}", line);
                    if let Ok(mut file) = stderr_log.lock() {
                        let _ = writeln!(file, "STDERR: {}", line);
                        let _ = file.flush();
                    }
                }
            }
        })
    });

    // Wait for both threads to complete
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }
}

/// Verify contract: exit code 0 means file must exist
pub fn verify_contract(
    log_file: &LogFile,
    status: std::process::ExitStatus,
    output_file: &PathBuf,
    platform: &str,
) {
    if status.success() {
        if output_file.exists() {
            log(log_file, &format!("✓ {} package created successfully", platform));
            log(log_file, &format!("  Location: {}", output_file.display()));
        } else {
            log(
                log_file,
                &format!(
                    "✗ {} package: CONTRACT VIOLATION - exit 0 but file missing",
                    platform
                ),
            );
            std::process::exit(1);
        }
    } else {
        log(
            log_file,
            &format!(
                "✗ {} package failed (exit code {:?})",
                platform,
                status.code()
            ),
        );
        std::process::exit(1);
    }
}
