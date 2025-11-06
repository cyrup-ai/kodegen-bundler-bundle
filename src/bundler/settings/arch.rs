//! CPU architecture types and utilities.

/// CPU architecture for target binaries.
///
/// Represents the target architecture for bundled binaries. The architecture is
/// automatically detected from the Rust target triple during bundling.
///
/// # Platform Support
///
/// - ✅ Linux: All architectures supported
/// - ✅ macOS: X86_64, AArch64, Universal
/// - ✅ Windows: X86_64, X86
///
/// # Examples
///
/// ```no_run
/// use kodegen_bundler_release::bundler::Arch;
///
/// let arch = Arch::X86_64;
/// println!("Target architecture: {:?}", arch);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    /// x86_64 / AMD64 (64-bit) - Most common desktop/server architecture
    X86_64,
    /// x86 / i686 (32-bit) - Legacy 32-bit Intel
    X86,
    /// AArch64 / ARM64 (64-bit) - Apple Silicon, modern ARM devices
    AArch64,
    /// ARM with hard-float (32-bit) - Raspberry Pi and embedded ARM
    Armhf,
    /// ARM with soft-float (32-bit) - Older embedded ARM devices
    Armel,
    /// RISC-V (64-bit) - Emerging open architecture
    Riscv64,
    /// macOS universal binary - Contains both x86_64 and AArch64
    Universal,
}
