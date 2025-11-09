//! Kodegen Bundler Bundle - Platform package bundler for Rust workspaces.
//!
//! This binary creates platform-specific packages (.deb, .rpm, .dmg, .msi, AppImage)
//! from Rust binaries with proper error handling and artifact verification.

mod bundler;
mod cli;
mod error;
mod metadata;
mod source;

use std::process;

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();

    // Run CLI and get exit code
    let exit_code = match cli::run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    };

    process::exit(exit_code);
}
