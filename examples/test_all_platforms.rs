//! Test bundler by building kodegen-tools-filesystem for all platforms
//!
//! This example tests the bundler binary (NOT the library) by invoking it as a subprocess
//! and building kodegen-tools-filesystem for every supported platform.
//!
//! All output is written to ./tmp/test_all_platforms.log
//!
//! Run with: cargo run --example test_all_platforms

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Helper to write to both console and log file (with flush for real-time output)
fn log(log_file: &Arc<Mutex<File>>, msg: &str) {
    println!("{}", msg);
    let mut file = log_file.lock().unwrap();
    writeln!(file, "{}", msg).expect("Failed to write to log");
    file.flush().expect("Failed to flush log");
}

fn main() {
    let platforms = ["deb", "rpm", "appimage", "dmg", "app", "nsis"];
    let output_dir = PathBuf::from("/tmp/kodegen-bundler-test");

    // Create ./tmp directory and log file
    std::fs::create_dir_all("./tmp").expect("Failed to create ./tmp directory");
    let log_file = File::create("./tmp/test_all_platforms.log").expect("Failed to create log file");
    let log_file = Arc::new(Mutex::new(log_file));

    log(&log_file, "Testing bundler for all platforms...");
    log(&log_file, &format!("Source: cyrup-ai/kodegen-tools-filesystem"));
    log(&log_file, &format!("Output directory: {}", output_dir.display()));
    log(&log_file, &format!("Log file: ./tmp/test_all_platforms.log\n"));

    let mut successes = Vec::new();

    for platform in &platforms {
        log(&log_file, "═══════════════════════════════════════");
        log(&log_file, &format!("Building {} package...", platform));
        log(&log_file, "═══════════════════════════════════════\n");

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
        let mut child = Command::new("cargo")
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
            .expect("Failed to spawn bundler");

        // Spawn thread to read stdout concurrently
        let stdout_log = Arc::clone(&log_file);
        let stdout_handle = child.stdout.take().map(|stdout| {
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let mut file = stdout_log.lock().unwrap();
                        writeln!(file, "{}", line).expect("Failed to write stdout to log");
                        file.flush().expect("Failed to flush log");
                    }
                }
            })
        });

        // Spawn thread to read stderr concurrently
        let stderr_log = Arc::clone(&log_file);
        let stderr_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let mut file = stderr_log.lock().unwrap();
                        writeln!(file, "STDERR: {}", line).expect("Failed to write stderr to log");
                        file.flush().expect("Failed to flush log");
                    }
                }
            })
        });

        // Wait for both threads to complete
        if let Some(handle) = stdout_handle {
            handle.join().expect("stdout thread panicked");
        }
        if let Some(handle) = stderr_handle {
            handle.join().expect("stderr thread panicked");
        }

        // Wait for process to complete
        let status = child.wait().expect("Failed to wait for bundler");

        // Contract verification: exit code 0 = file guaranteed to exist
        if status.success() {
            if output_file.exists() {
                log(&log_file, &format!("✓ {} package created successfully", platform));
                log(&log_file, &format!("  Location: {}", output_file.display()));
                successes.push(*platform);
            } else {
                log(&log_file, &format!(
                    "✗ {} package: CONTRACT VIOLATION - exit 0 but file missing",
                    platform
                ));
                log(&log_file, "\n✗ Terminating due to contract violation");
                std::process::exit(1);
            }
        } else {
            log(&log_file, &format!("✗ {} package failed (exit code {:?})", platform, status.code()));
            log(&log_file, "\n✗ Terminating due to build failure");
            std::process::exit(1);
        }

        log(&log_file, "");
    }

    // Summary
    log(&log_file, "═══════════════════════════════════════");
    log(&log_file, "SUMMARY");
    log(&log_file, "═══════════════════════════════════════\n");
    log(&log_file, &format!("Successes: {:?}", successes));

    if successes.len() == platforms.len() {
        log(&log_file, "\n✓ All platforms bundled successfully!");
        std::process::exit(0);
    } else {
        // This branch is unreachable - program exits on first failure
        log(&log_file, "\n✗ Build terminated on first failure");
        std::process::exit(1);
    }
}
