# Multi-stage Dockerfile for incremental testing
# Test stages independently to catch failures early:
#   docker build --target=base-packages     (30 seconds)
#   docker build --target=rust-toolchain    (5 minutes)
#   docker build --target=bundler-builder   (10 minutes - includes bundler build)
#   docker build .                          (full build)

# ============================================================================
# Stage 1: base-packages
# Quick test: package installation
# Test with: docker build --target=base-packages -t kodegen-test-base .
# ============================================================================
FROM rust:1.83-bookworm AS base-packages

# Install system dependencies for building all platforms
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl \
    wget \
    unzip \
    # NSIS native Linux compiler for Windows installers
    nsis \
    # Windows cross-compilation toolchain (for building Windows binaries on Linux)
    gcc-mingw-w64-x86-64 \
    g++-mingw-w64-x86-64 \
    # Linux package tools
    rpm \
    fakeroot \
    dpkg-dev \
    # AppImage dependencies
    file \
    desktop-file-utils \
    fuse \
    libfuse2 \
    squashfs-tools \
    # General utilities
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Verify base packages installed correctly
RUN makensis -VERSION

# ============================================================================
# Stage 2: user-setup
# Create build user with proper permissions
# ============================================================================
FROM base-packages AS user-setup

# Create build user matching typical host UID/GID
ARG USER_UID=1000
ARG USER_GID=1000

RUN groupadd --gid $USER_GID builder \
    && useradd --uid $USER_UID --gid $USER_GID -m builder \
    && apt-get update \
    && apt-get install -y sudo \
    && echo builder ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/builder \
    && chmod 0440 /etc/sudoers.d/builder \
    && rm -rf /var/lib/apt/lists/*

# Switch to build user
USER builder
ENV HOME=/home/builder
ENV CARGO_HOME=$HOME/.cargo
ENV PATH=$HOME/.cargo/bin:$PATH

# ============================================================================
# Stage 3: rust-toolchain
# Install Rust toolchain and components
# Test with: docker build --target=rust-toolchain -t kodegen-test-rust .
# ============================================================================
FROM user-setup AS rust-toolchain

# Switch to root to make Rust toolchain accessible to all UIDs
USER root

# Install Rust nightly with common targets and components
RUN rustup default nightly \
    && rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown x86_64-apple-darwin x86_64-pc-windows-gnu \
    && chmod -R a+rX /usr/local/rustup /usr/local/cargo

# Switch back to builder user
USER builder

# Verify Rust installation
RUN rustc --version && cargo --version

# ============================================================================
# Stage 4: bundler-builder
# Build and install bundler binary during image creation
# ============================================================================
FROM rust-toolchain AS bundler-builder

# Install bundler from GitHub main branch (always latest)
# Cache bust 2025-01-07: Removed --version CLI parameter
RUN cargo install \
    --git https://github.com/cyrup-ai/kodegen-bundler-bundle \
    --branch main \
    --bin kodegen_bundler_bundle \
    kodegen_bundler_bundle

# Create shared cargo home in /tmp with sticky bit for multi-user access
# Container runs with --user ${host_uid}:${host_gid}, we need a truly shared cache
RUN mkdir -p /tmp/cargo && chmod 1777 /tmp/cargo

# Verify bundler installation
RUN kodegen_bundler_bundle --version || kodegen_bundler_bundle --help

# ============================================================================
# Stage 5: full-builder (default)
# Complete build environment ready for compilation with pre-built bundler
# ============================================================================
FROM bundler-builder AS full-builder

# Create cache directory for NSIS downloads
RUN mkdir -p $HOME/.cache/cyrup

# Set working directory for runtime
WORKDIR /workspace

# Final verification of all tools including pre-built bundler
RUN rustc --version && \
    cargo --version && \
    makensis -VERSION && \
    kodegen_bundler_bundle --help

# Build instructions:
# - Stage testing: docker build --target=<stage> -t kodegen-test-<stage> .
# - Full image: docker build -t kodegen-release-builder .
# - With cache: docker build --pull -t kodegen-release-builder .
