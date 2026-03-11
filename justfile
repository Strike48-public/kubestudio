# ============ Docker (Multi-Arch Scratch) ============

# Build multi-arch scratch image locally (loads into docker)
docker-build tag="latest":
    docker buildx build \
        --platform linux/amd64 \
        --load \
        -t ghcr.io/strike48-public/kubestudio:{{tag}} \
        -f Dockerfile.scratch .

# Build and inspect Dockerfile.scratch (dry-run to see layers)
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
