# ============ Docker (Multi-Arch Distroless) ============
#
# Canonical Dockerfile is `Dockerfile.scratch` (distroless/cc + tini + static
# busybox). CI publishes multi-arch via .github/workflows/docker-multiarch.yml
# on push to main and on `v*` tag.
#
# `Dockerfile` (Debian + tini) is retained as a developer-friendly local build
# with a full shell for interactive debugging — not published by CI.

# Build the canonical distroless image locally (loads into docker)
docker-build tag="latest":
    docker buildx build \
        --platform linux/amd64 \
        --load \
        -t ghcr.io/strike48-public/kubestudio:{{tag}} \
        -f Dockerfile.scratch .

# Build and inspect the canonical distroless image
docker-package:
    @echo "=== Dockerfile.scratch ==="
    @cat Dockerfile.scratch
    @echo ""
    @echo "=== Building locally (amd64 only, for inspection) ==="
    docker buildx build \
        --platform linux/amd64 \
        --load \
        -t kubestudio:local-scratch \
        -f Dockerfile.scratch .
    @echo ""
    @echo "=== Image details ==="
    docker images kubestudio:local-scratch
    @echo ""
    @echo "=== Image layers ==="
    docker history kubestudio:local-scratch
