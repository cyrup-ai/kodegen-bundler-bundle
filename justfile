# Justfile for kodegen-bundler-bundle

# Docker cache volumes for faster builds
CARGO_CACHE_VOLUME := "kodegen-bundler-cargo-cache"
TARGET_CACHE_VOLUME := "kodegen-bundler-target-cache"

# Create cache volumes (one-time setup)
create-cache-volumes:
    @echo "Creating Docker cache volumes..."
    @docker volume create {{CARGO_CACHE_VOLUME}} || true
    @docker volume create {{TARGET_CACHE_VOLUME}} || true
    @echo "✓ Cache volumes ready"

# Rebuild Docker image (force rebuild)
rebuild-image:
    @echo "🔨 Rebuilding Docker image..."
    docker build --no-cache -t kodegen-release-builder .devcontainer/

# Show help
help:
    @echo "kodegen-bundler-bundle commands:"
    @echo ""
    @echo "  just create-cache-volumes   - Create Docker cache volumes (one-time)"
    @echo "  just rebuild-image          - Force rebuild Docker image"
