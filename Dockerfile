# ============================================================
# KubeStudio — multi-stage Docker build with cargo-chef caching
# ============================================================
# Builds:
#   ks-server     — standalone web UI
#   ks-connector  — Matrix AI connector
#
# Usage:
#   docker build -t kubestudio .
#   docker run -p 8080:8080 \
#     -v ~/.kube/config:/etc/kubestudio/kubeconfigs/default:ro \
#     -e KUBECONFIG=/etc/kubestudio/kubeconfigs/default \
#     kubestudio

# ----------------------------------------------------------
# Stage 1: cargo-chef planner — compute dependency recipe
# ----------------------------------------------------------
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ----------------------------------------------------------
# Stage 2: cargo-chef cook — build dependencies (cached layer)
# ----------------------------------------------------------
FROM chef AS builder

# Install build-time system deps (OpenSSL headers for kube-rs)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
# Pre-cache deps with combined feature set (single cook pass)
RUN cargo chef cook --release --recipe-path recipe.json \
    --features server,connector --no-default-features -p ks-ui

# ----------------------------------------------------------
# Stage 3: Build the actual binaries
# ----------------------------------------------------------
COPY . .

# Build both binaries in one pass — cargo shares compilation artifacts
RUN cargo build --release \
    --bin ks-server --bin ks-connector \
    --features server,connector --no-default-features -p ks-ui

# ----------------------------------------------------------
# Stage 4: Minimal runtime image
# ----------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN groupadd --gid 999 kubestudio \
    && useradd --uid 999 --gid kubestudio --shell /bin/false kubestudio

COPY --from=builder /app/target/release/ks-server /usr/local/bin/ks-server
COPY --from=builder /app/target/release/ks-connector /usr/local/bin/ks-connector

# Kubeconfig mount point
RUN mkdir -p /etc/kubestudio/kubeconfigs && \
    chown -R kubestudio:kubestudio /etc/kubestudio

USER kubestudio

ENV PORT=8080
EXPOSE 8080

ENTRYPOINT ["tini", "--"]
CMD ["ks-server"]
