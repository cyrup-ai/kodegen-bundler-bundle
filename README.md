# kodegen-bundler-bundle

**Multi-platform package bundler for Rust applications.**

[![License](https://img.shields.io/badge/license-Apache%202.0%20OR%20MIT-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-nightly--2024--10--20-orange.svg)](https://rust-lang.github.io/rustup/)

## Overview

`kodegen-bundler-bundle` is a standalone binary that creates platform-specific installation packages for Rust applications. It supports Linux (.deb, .rpm, AppImage), macOS (.app, .dmg), and Windows (.msi, .exe) package formats.

This bundler is designed to be called programmatically by release workflows (like `kodegen-bundler-release`) with explicit output path contracts.

## Features

- 📦 **Multi-Platform Support** - Linux (deb/rpm/AppImage), macOS (app/dmg), Windows (msi/exe)
- 🎯 **Caller-Specified Output Paths** - Full control over artifact location and naming
- 🔒 **Contract-Based Interface** - Exit code 0 guarantees artifact exists at specified path
- 🚀 **Fast Execution** - Optimized bundling with minimal overhead
- 🛡️ **Directory Management** - Automatic parent directory creation
- 📊 **Diagnostic Output** - Detailed stdout/stderr for debugging

## Installation

```bash
# Install from crates.io
cargo install kodegen_bundler_bundle

# OR build from source
git clone https://github.com/cyrup-ai/kodegen-bundler-bundle
cd kodegen-bundler-bundle
cargo install --path .
```

## Basic Usage

The bundler accepts exactly **three arguments** and handles everything else internally:

```bash
# Bundle from local repository (reads Cargo.toml for GitHub URL, clones to tmp, builds, bundles)
kodegen_bundler_bundle \
  --source . \
  --platform deb \
  --output-binary /tmp/artifacts/myapp_1.0.0_arm64.deb

# Bundle from GitHub org/repo (clones to tmp, builds, bundles)
kodegen_bundler_bundle \
  --source cyrup-ai/kodegen \
  --platform dmg \
  --output-binary /tmp/artifacts/kodegen_0.1.2_arm64.dmg

# Bundle from GitHub URL (clones to tmp, builds, bundles)
kodegen_bundler_bundle \
  --source https://github.com/cyrup-ai/kodegen \
  --platform nsis \
  --output-binary C:\builds\kodegen_setup.exe
```

**Exit code 0 = artifact guaranteed to exist at `--output-binary` path.**

## Output Path Contract

### The `--output-binary` Flag

When `--output-binary` is specified, the bundler establishes a **strict contract** with the caller:

#### Bundler Responsibilities

1. **Create parent directories** - All directories in the output path are created if they don't exist
2. **Move artifact** - The created artifact is moved (not copied) to the exact specified path
3. **Verify existence** - Before returning, bundler verifies the file exists at the specified path
4. **Return exit code 0** - Exit code 0 **guarantees** the file exists at the specified path

#### Contract Guarantees

```
If bundler returns exit code 0:
  ✓ File exists at --output-binary path
  ✓ File is complete and valid
  ✓ All parent directories created
  ✓ Original artifact removed from temp location

If bundler returns non-zero exit code:
  ✗ File may not exist at specified path
  ✗ Check stderr for error details
```

#### Communication Protocol

- **Exit codes**: Contractual communication (0 = success, file exists; non-zero = failure)
- **stdout**: Diagnostic information only (artifact paths, progress messages)
- **stderr**: Error details and warnings (diagnostic only, not contractual)

**Important**: Callers should **only** rely on exit codes for contract enforcement. stdout and stderr are for human consumption and debugging, not programmatic parsing.

## CLI Reference

### The Three Required Arguments

```bash
--source <SOURCE>           # Where to get the code (3 formats):
                           # 1. Local path: . or /path/to/repo
                           #    → Reads Cargo.toml repository field
                           #    → Clones from GitHub to /tmp/kodegen-bundle-{uuid}
                           # 2. GitHub org/repo: cyrup-ai/kodegen
                           #    → Clones from GitHub to /tmp/kodegen-bundle-{uuid}
                           # 3. GitHub URL: https://github.com/cyrup-ai/kodegen
                           #    → Clones from GitHub to /tmp/kodegen-bundle-{uuid}
                           # 
                           # ALL sources clone to tmp - NEVER builds in-place

--platform <PLATFORM>       # Target platform: deb, rpm, appimage, dmg, nsis

--output-binary <PATH>      # Full output path for final artifact
                           # Example: /tmp/artifacts/myapp_1.0.0_arm64.deb
                           # Bundler creates parent dirs automatically
                           # Exit code 0 guarantees file exists at this path
```

### What the Bundler Handles Internally

The bundler automatically:
- Clones repository to tmp directory (`/tmp/kodegen-bundle-{uuid}`)
- Reads binary name from Cargo.toml
- Reads version from Cargo.toml
- Detects target architecture
- Builds the binary (`cargo build --release`)
- Creates the platform package
- Moves artifact to `--output-binary` path
- Cleans up tmp directory
- Returns exit code 0 only if artifact exists

**Caller responsibilities**: Specify source, platform, output path
**Bundler responsibilities**: Everything else

## Supported Platforms

| Platform | Extension | Description |
|----------|-----------|-------------|
| `deb` | `.deb` | Debian/Ubuntu packages |
| `rpm` | `.rpm` | RedHat/Fedora/CentOS packages |
| `appimage` | `.AppImage` | Portable Linux executables |
| `dmg` | `.dmg` | macOS disk image installers |
| `app` | `.app` | macOS application bundles |
| `nsis` | `.exe` | Windows NSIS installers |

## Usage Examples

### Bundle from Local Repository

```bash
# Reads repository URL from Cargo.toml, clones to tmp, builds, bundles
kodegen_bundler_bundle \
  --source /path/to/project \
  --platform deb \
  --output-binary /tmp/artifacts/myapp_1.0.0_amd64.deb

# Exit code 0 = file guaranteed at /tmp/artifacts/myapp_1.0.0_amd64.deb
```

### Bundle from GitHub Org/Repo

```bash
# Clones cyrup-ai/kodegen from GitHub to tmp, builds, bundles
kodegen_bundler_bundle \
  --source cyrup-ai/kodegen \
  --platform deb \
  --output-binary /tmp/artifacts/kodegen_2.0.0_arm64.deb

# Exit code 0 = file guaranteed at /tmp/artifacts/kodegen_2.0.0_arm64.deb
```

### Bundle from GitHub URL

```bash
# Clones from full GitHub URL to tmp, builds, bundles
kodegen_bundler_bundle \
  --source https://github.com/cyrup-ai/kodegen \
  --platform dmg \
  --output-binary ./dist/kodegen-3.1.4-arm64.dmg

# Bundler automatically creates ./dist/ directory
```

### Cross-Platform Bundling

```bash
# Build Linux package from macOS (uses Docker internally)
kodegen_bundler_bundle \
  --source . \
  --platform deb \
  --output-binary /tmp/myapp.deb

# Build Windows installer from Linux (uses Docker with Wine)
kodegen_bundler_bundle \
  --source cyrup-ai/myapp \
  --platform nsis \
  --output-binary C:\builds\myapp_setup.exe
```

## Integration with Release Workflows

The bundler is designed to integrate seamlessly with release automation tools like `kodegen-bundler-release`.

### Typical Workflow Integration

1. **Release workflow detects target architecture** (compile-time cfg attributes)
2. **Release constructs output path** with explicit architecture in filename
3. **Release invokes bundler** with `--output-binary` flag
4. **Bundler creates directories** and moves artifact to specified path
5. **Bundler returns exit 0** only if file exists
6. **Release verifies contract** by checking file existence

### Example: Release Workflow Calling Bundler

```rust
// In kodegen-bundler-release/src/cli/commands/release/impl.rs

// Release workflow constructs output path with architecture
let arch = detect_target_architecture()?;  // "arm64", "amd64", etc.
let version = "2.0.0";  // From Cargo.toml (bundler reads this internally too)
let filename = format!("kodegen_{}_{}.deb", version, arch);
let output_path = artifacts_dir.join(&filename);

// Call bundler with three arguments
let output = Command::new("kodegen_bundler_bundle")
    .arg("--source").arg("cyrup-ai/kodegen")  // ← GitHub org/repo
    .arg("--platform").arg("deb")
    .arg("--output-binary").arg(&output_path)
    .output()?;

// Contract enforcement: exit 0 = file exists
if output.status.success() {
    if !output_path.exists() {
        return Err("Bundler contract violation: exit 0 but file missing");
    }
    // File guaranteed to exist at output_path
    // Bundler handled: cloning, building, bundling, cleanup
}
```

## Architecture Handling

### Caller Responsibility

The **caller** (e.g., release workflow) is responsible for:
- Specifying source (local path, GitHub org/repo, or GitHub URL)
- Specifying target platform (deb, rpm, dmg, etc.)
- Constructing the output filename with architecture (e.g., `myapp_1.0.0_arm64.deb`)
- Passing the complete output path to bundler

### Bundler Responsibility

The **bundler** handles everything else:
- Cloning repository to `/tmp/kodegen-bundle-{uuid}` from GitHub
- Reading binary name from Cargo.toml
- Reading version from Cargo.toml  
- Detecting target architecture
- Building the binary (`cargo build --release`)
- Creating the platform package
- Creating parent directories in the output path
- Moving the artifact to the specified location
- Cleaning up tmp directory
- Verifying the file exists before returning exit 0

### Why This Design?

This separation ensures:
- **Caller** only needs to know: source location, platform, and output path
- **Bundler** is completely self-contained and handles all implementation details
- **Contract** is enforced through exit codes: 0 = file exists, non-zero = failure
- **Future-proof** for new platforms and architectures (no caller changes needed)
- **Isolation** - builds always happen in tmp, never modifies source directory

## Error Handling

### Exit Codes

| Exit Code | Meaning |
|-----------|---------|
| `0` | Success - if --output-binary specified, file guaranteed to exist |
| `1` | General error - check stderr |
| Non-zero | Specific error - check stderr for details |

### Common Errors

#### Directory Creation Failed

```
Error: Failed to create output directory /path/to/output: Permission denied
```

**Solution**: Check write permissions on the parent directory.

#### Artifact Move Failed

```
Error: Failed to move artifact from /tmp/bundle.deb to /output/app.deb: No such file or directory
```

**Solution**: Verify source artifact was created successfully. Check bundler logs.

#### Contract Violation

```
Error: Move reported success but file does not exist at /output/app.deb
```

**Solution**: This indicates a bundler bug. Report to maintainers.

## Required Project Structure

For bundling to work, your project must have:

```
your-project/
├── Cargo.toml                    # [package.metadata.bundle] section
├── src/
│   └── main.rs
├── assets/
│   └── img/
│       ├── icon.icns             # macOS
│       ├── icon.ico              # Windows
│       └── icon_*x*.png          # Linux (multiple sizes)
└── target/
    └── release/
        └── your-binary           # Built binary
```

See [kodegen-bundler-release README](../kodegen-bundler-release/README.md) for detailed asset requirements.

## Configuration

### TOML Configuration Structure

Bundle settings are configured in your project's `Cargo.toml` under `[package.metadata.bundle]`. The configuration uses a **flat structure** where platform-specific settings are direct children of the bundle section.

**Important**: Platform settings like `deb`, `rpm`, `appimage`, `macos`, and `windows` are **direct fields** under `[package.metadata.bundle]`, not nested under intermediate sections like `.linux`.

### Complete Configuration Example

```toml
[package.metadata.bundle]
# Universal settings
identifier = "com.example.myapp"
publisher = "Example Inc."
icon = ["assets/img/icon_32x32.png", "assets/img/icon_128x128.png"]
resources = ["assets/data"]
copyright = "Copyright © 2025 Example Inc."
category = "Utility"
short_description = "My awesome application"
long_description = "A detailed description of my application"

# Linux: Debian/Ubuntu packages
[package.metadata.bundle.deb]
depends = ["libc6 (>= 2.31)", "libssl3"]
section = "utils"
priority = "optional"

# Linux: RedHat/Fedora/CentOS packages
[package.metadata.bundle.rpm]
depends = ["glibc >= 2.31", "openssl-libs"]
release = "1"

# Linux: AppImage portable executables
[package.metadata.bundle.appimage]
bins = ["myapp"]

# macOS: Application bundles and disk images
[package.metadata.bundle.macos]
frameworks = []
minimum_system_version = "10.13"
signing_identity = "Developer ID Application: Example Inc. (TEAM123)"

[package.metadata.bundle.macos.dmg]
background = "assets/dmg-background.png"
window_size = { width = 660, height = 400 }

# Windows: MSI and NSIS installers
[package.metadata.bundle.windows]
wix_language = "en-US"

[package.metadata.bundle.windows.nsis]
installer_mode = "perUser"
compression = "lzma"
```

### Platform-Specific Configuration Details

#### Debian Packages (`[package.metadata.bundle.deb]`)

```toml
[package.metadata.bundle.deb]
depends = ["libc6 (>= 2.31)"]    # Runtime dependencies
section = "utils"                 # Package category
priority = "optional"             # Installation priority
```

**Note**: The path is `[package.metadata.bundle.deb]`, **not** `[package.metadata.bundle.linux.deb]`.

#### RPM Packages (`[package.metadata.bundle.rpm]`)

```toml
[package.metadata.bundle.rpm]
depends = ["glibc >= 2.31"]      # Runtime dependencies
release = "1"                     # RPM release number
```

**Note**: The path is `[package.metadata.bundle.rpm]`, **not** `[package.metadata.bundle.linux.rpm]`.

#### AppImage (`[package.metadata.bundle.appimage]`)

```toml
[package.metadata.bundle.appimage]
bins = ["myapp", "myapp-cli"]    # Binaries to include
```

**Note**: The path is `[package.metadata.bundle.appimage]`, **not** `[package.metadata.bundle.linux.appimage]`.

#### macOS Bundles (`[package.metadata.bundle.macos]`)

```toml
[package.metadata.bundle.macos]
frameworks = []                                    # Additional frameworks
minimum_system_version = "10.13"                   # Minimum macOS version
signing_identity = "Developer ID Application: ..." # Code signing identity

[package.metadata.bundle.macos.dmg]
background = "assets/dmg-background.png"           # DMG background image
window_size = { width = 660, height = 400 }        # DMG window size
```

#### Windows Installers (`[package.metadata.bundle.windows]`)

```toml
[package.metadata.bundle.windows]
wix_language = "en-US"           # MSI installer language

[package.metadata.bundle.windows.nsis]
installer_mode = "perUser"        # "perUser" or "perMachine"
compression = "lzma"              # "none", "zlib", or "lzma"
```

### Minimal Configuration

The bundler works with minimal configuration, using sensible defaults:

```toml
[package.metadata.bundle]
identifier = "com.example.myapp"
publisher = "Example Inc."
icon = ["assets/img/icon.png"]
```

All platform-specific sections are optional and will use defaults if not specified.

### TOML Path Reference

**Correct paths** for platform-specific configuration:

- ✅ `[package.metadata.bundle.deb]` - Debian settings
- ✅ `[package.metadata.bundle.rpm]` - RPM settings
- ✅ `[package.metadata.bundle.appimage]` - AppImage settings
- ✅ `[package.metadata.bundle.macos]` - macOS settings
- ✅ `[package.metadata.bundle.macos.dmg]` - DMG-specific settings
- ✅ `[package.metadata.bundle.windows]` - Windows settings
- ✅ `[package.metadata.bundle.windows.nsis]` - NSIS-specific settings

**Incorrect paths** (do not use):

- ❌ `[package.metadata.bundle.linux.deb]` - Wrong, no `.linux` parent
- ❌ `[package.metadata.bundle.linux.rpm]` - Wrong, no `.linux` parent
- ❌ `[package.metadata.bundle.linux.appimage]` - Wrong, no `.linux` parent

## Building from Source

### Prerequisites

- **Rust nightly** (edition 2024): `rustup install nightly && rustup default nightly`
- **Platform-specific tools**:
  - **Linux**: dpkg-dev, rpm, fakeroot
  - **macOS**: Xcode Command Line Tools
  - **Windows**: WiX Toolset, NSIS

### Build Commands

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Format and lint
cargo fmt
cargo clippy -- -D warnings
```

## License

Dual-licensed under **Apache-2.0 OR MIT**.

See [LICENSE.md](LICENSE.md) for details.

## Credits

Part of the [KODEGEN.ᴀɪ](https://kodegen.ai) project - blazing-fast MCP tools for AI-powered code generation.

## Support

- **Issues**: [GitHub Issues](https://github.com/cyrup-ai/kodegen-bundler-bundle/issues)
- **Documentation**: [docs.rs](https://docs.rs/kodegen_bundler_bundle)
- **Website**: [kodegen.ai](https://kodegen.ai)
