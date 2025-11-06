//! Code signing setup and certificate management.
//!
//! This module handles code signing setup, particularly for macOS where
//! certificates need to be imported from environment variables for CI/CD.

use crate::bundler::Result;

/// Setup macOS code signing from environment variables
///
/// This function handles certificate import from environment variables for CI/CD.
/// - APPLE_CERTIFICATE: Base64-encoded .p12 certificate imported to temp keychain
/// - APPLE_API_KEY, APPLE_API_ISSUER: Used directly by xcrun notarytool (no file needed)
///
/// The TempKeychain is kept alive for the lifetime of the Bundler, ensuring
/// the certificate remains available for all signing operations.
#[cfg(target_os = "macos")]
pub async fn setup_macos_signing() -> Result<Option<kodegen_bundler_sign::macos::TempKeychain>> {
    // Note: API key env vars (APPLE_API_KEY, APPLE_API_ISSUER, APPLE_API_KEY_CONTENT)
    // are used directly by xcrun notarytool - no need to write .p8 files

    // Import certificate if APPLE_CERTIFICATE is set
    if let (Ok(cert_b64), Ok(password)) = (
        std::env::var("APPLE_CERTIFICATE"),
        std::env::var("APPLE_CERTIFICATE_PASSWORD").map(|p| p.trim().to_string()),
    ) {
        use base64::Engine;
        let cert_bytes = base64::engine::general_purpose::STANDARD
            .decode(cert_b64)
            .map_err(|e| {
                crate::bundler::Error::GenericError(format!(
                    "Invalid APPLE_CERTIFICATE (not valid base64): {}",
                    e
                ))
            })?;

        log::info!("Importing certificate from APPLE_CERTIFICATE environment variable");
        let keychain = kodegen_bundler_sign::macos::TempKeychain::from_certificate_bytes(
            &cert_bytes,
            &password,
        )
        .await
        .map_err(|e| {
            crate::bundler::Error::GenericError(format!("Failed to import certificate: {}", e))
        })?;

        log::info!("✓ Certificate imported to temporary keychain");
        return Ok(Some(keychain));
    }

    Ok(None)
}
