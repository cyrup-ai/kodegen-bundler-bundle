//! Test bundler for EXE platform
//!
//! This example tests the bundler binary by invoking it as a subprocess
//! and building a Windows NSIS installer.
//!
//! Output written to ./tmp/test_exe.log
//! Bundle artifact: ./tmp/bundle/{package_id}.exe
//!
//! Run with: cargo run --example exe -- --source owner/repo

mod common;

fn main() {
    let platform = "exe";

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let source = if let Some(pos) = args.iter().position(|arg| arg == "--source") {
        if pos + 1 < args.len() {
            args[pos + 1].clone()
        } else {
            eprintln!("Error: --source requires a value (e.g., owner/repo)");
            std::process::exit(1);
        }
    } else {
        eprintln!("Usage: cargo run --example exe -- --source owner/repo");
        std::process::exit(1);
    };

    // Extract package_id from source (e.g., "cyrup-ai/kodegen-tools-filesystem" -> "kodegen-tools-filesystem")
    let package_id = source.split('/').last().unwrap_or(&source);

    // Clean up previous artifacts and logs
    common::cleanup(platform, package_id);

    // Setup log file
    let log_file = common::setup_log_file(platform).expect("Failed to create log file");

    common::log(&log_file, "═══════════════════════════════════════");
    common::log(&log_file, &format!("Testing {} bundler", platform.to_uppercase()));
    common::log(&log_file, "═══════════════════════════════════════\n");
    common::log(&log_file, &format!("Source: {}", source));
    common::log(&log_file, &format!("Log file: ./tmp/test_{}.log", platform));

    // Construct output path in ./tmp/bundle
    let extension = common::platform_extension(platform);
    let output_file = common::construct_output_path(package_id, extension)
        .expect("Failed to construct output path");
    common::log(&log_file, &format!("Bundle output: {}\n", output_file.display()));

    // Spawn bundler subprocess
    let mut child = common::spawn_bundler(platform, &output_file, &source)
        .expect("Failed to spawn bundler");

    // Stream output to console and log file
    common::stream_output(&mut child, &log_file);

    // Wait for process and verify contract
    let status = child.wait().expect("Failed to wait for bundler");
    common::verify_contract(&log_file, status, &output_file, platform);

    common::log(&log_file, "\n✓ Test completed successfully!");
}
