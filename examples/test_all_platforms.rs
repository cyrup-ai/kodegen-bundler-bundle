//! Test bundler by building kodegen-tools-filesystem for all platforms
//!
//! This example tests the bundler binary (NOT the library) by invoking it as a subprocess
//! and building kodegen-tools-filesystem for every supported platform.
//!
//! All output is written to ./tmp/test_all_platforms.log
//!
//! Run with: cargo run --example test_all_platforms

use kodegen_bundler_bundle::cli::OutputManager;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Helper to write to both console and log file (with flush for real-time output)
fn log(output: &OutputManager, log_file: &Arc<Mutex<File>>, msg: &str) {
    output.info(msg);
    if let Ok(mut file) = log_file.lock() {
        let _ = writeln!(file, "{}", msg);
        let _ = file.flush();
    }
}

fn main() {
    let output = OutputManager::new(true, false);
    let platforms = ["deb", "rpm", "appimage", "dmg", "app", "nsis"];
    let output_dir = PathBuf::from("/tmp/kodegen-bundler-test");

    // Create ./tmp directory and log file
    if let Err(e) = std::fs::create_dir_all("./tmp") {
        output.error(&format!("Failed to create ./tmp directory: {}", e));
        std::process::exit(1);
    }
    let log_file = match File::create("./tmp/test_all_platforms.log") {
        Ok(file) => file,
        Err(e) => {
            output.error(&format!("Failed to create log file: {}", e));
            std::process::exit(1);
        }
    };
    let log_file = Arc::new(Mutex::new(log_file));

    log(&output, &log_file, "Testing bundler for all platforms...");
    log(&output, &log_file, &format!("Source: cyrup-ai/kodegen-tools-filesystem"));
    log(&output, &log_file, &format!("Output directory: {}", output_dir.display()));
    log(&output, &log_file, &format!("Log file: ./tmp/test_all_platforms.log\n"));

    let mut successes = Vec::new();

    for platform in &platforms {
        log(&output, &log_file, "═══════════════════════════════════════");
        log(&output, &log_file, &format!("Building {} package...", platform));
        log(&output, &log_file, "═══════════════════════════════════════\n");

        // Construct output filename based on platform
        let extension = match *platform {
            "deb" => "deb",
            "rpm" => "rpm",
            "appimage" => "AppImage",
            "dmg" => "dmg",
            "app" => "app",
            "nsis" => "exe",
            _ => platform,
        };
        let output_file = output_dir.join(format!("kodegen-tools-filesystem.{}", extension));

        // Invoke bundler binary as subprocess with real-time output streaming
        let mut child = match Command::new("cargo")
            .args(["run", "--release", "--"])
            .arg("--source")
            .arg("cyrup-ai/kodegen-tools-filesystem")
            .arg("--platform")
            .arg(platform)
            .arg("--output-binary")
            .arg(&output_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                log(&output, &log_file, &format!("✗ Failed to spawn bundler: {}", e));
                std::process::exit(1);
            }
        };

        // Spawn thread to read stdout concurrently
        let stdout_log = Arc::clone(&log_file);
        let stdout_output = output.clone();
        let stdout_handle = child.stdout.take().map(|stdout| {
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        stdout_output.indent(&line);
                        if let Ok(mut file) = stdout_log.lock() {
                            let _ = writeln!(file, "{}", line);
                            let _ = file.flush();
                        }
                    }
                }
            })
        });

        // Spawn thread to read stderr concurrently
        let stderr_log = Arc::clone(&log_file);
        let stderr_output = output.clone();
        let stderr_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        stderr_output.indent(&line);
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

        // Wait for process to complete
        let status = match child.wait() {
            Ok(status) => status,
            Err(e) => {
                log(&output, &log_file, &format!("✗ Failed to wait for bundler: {}", e));
                std::process::exit(1);
            }
        };

        // Contract verification: exit code 0 = file guaranteed to exist
        if status.success() {
            if output_file.exists() {
                log(&output, &log_file, &format!("✓ {} package created successfully", platform));
                log(&output, &log_file, &format!("  Location: {}", output_file.display()));
                successes.push(*platform);
            } else {
                log(&output, &log_file, &format!(
                    "✗ {} package: CONTRACT VIOLATION - exit 0 but file missing",
                    platform
                ));
                log(&output, &log_file, "\n✗ Terminating due to contract violation");
                std::process::exit(1);
            }
        } else {
            log(&output, &log_file, &format!("✗ {} package failed (exit code {:?})", platform, status.code()));
            log(&output, &log_file, "\n✗ Terminating due to build failure");
            std::process::exit(1);
        }

        log(&output, &log_file, "");
    }

    // Summary
    log(&output, &log_file, "═══════════════════════════════════════");
    log(&output, &log_file, "SUMMARY");
    log(&output, &log_file, "═══════════════════════════════════════\n");
    log(&output, &log_file, &format!("Successes: {:?}", successes));

    if successes.len() == platforms.len() {
        log(&output, &log_file, "\n✓ All platforms bundled successfully!");
        std::process::exit(0);
    } else {
        // This branch is unreachable - program exits on first failure
        log(&output, &log_file, "\n✗ Build terminated on first failure");
        std::process::exit(1);
    }
}
