#!/usr/bin/env bash
# Build and run the KubeStudio connector against Strike48 Studio (or a local stack with --local).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/run-connector.sh [--local]

  (default)  Connect to studio.strike48.com. TENANT_ID is required.
  --local    Connect to a local Strike48 stack on *.strike48.test with TLS
             verification disabled. TENANT_ID defaults to non-prod.

Environment overrides: TENANT_ID, INSTANCE_ID, CONNECTOR_NAME, KUBESTUDIO_MODE,
KUBESTUDIO_AI, RUST_LOG. See docs/running-connector.md.
EOF
}

mode=prod
case "${1:-}" in
  "") ;;
  --local) mode=local ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

export RUST_LOG="${RUST_LOG:-info}"
export KUBESTUDIO_AI="${KUBESTUDIO_AI:-true}"
export KUBESTUDIO_MODE="${KUBESTUDIO_MODE:-read}"

if [[ $mode == local ]]; then
  export STRIKE48_API_URL="https://studio.strike48.test"
  export STRIKE48_URL="grpc://connectors-studio.strike48.test:80"
  export TENANT_ID="${TENANT_ID:-non-prod}"
  export INSTANCE_ID="${INSTANCE_ID:-kubestudio-local}"
  export MATRIX_TLS_INSECURE=true
else
  export STRIKE48_API_URL="https://studio.strike48.com"
  export STRIKE48_URL="grpc://connectors.studio.strike48.com:443"
  if [[ -z ${TENANT_ID:-} ]]; then
    echo "ERROR: set TENANT_ID to your Strike48 tenant, e.g. TENANT_ID=my-org $0" >&2
    exit 1
  fi
  export INSTANCE_ID="${INSTANCE_ID:-kubestudio-$(hostname -s)}"
  export MATRIX_TLS_INSECURE="${MATRIX_TLS_INSECURE:-false}"
fi

kubeconfig="${KUBECONFIG:-$HOME/.kube/config}"
if [[ ! -f ${kubeconfig%%:*} ]]; then
  echo "ERROR: no kubeconfig at ${kubeconfig%%:*}" >&2
  exit 1
fi

echo "KubeStudio connector ($mode)"
echo "  API URL:    $STRIKE48_API_URL"
echo "  gRPC URL:   $STRIKE48_URL"
echo "  Tenant:     $TENANT_ID"
echo "  Instance:   $INSTANCE_ID"
echo "  Mode:       $KUBESTUDIO_MODE"
echo "  TLS verify: $([[ $MATRIX_TLS_INSECURE == true ]] && echo disabled || echo enabled)"
if command -v kubectl >/dev/null 2>&1; then
  echo "  Context:    $(kubectl config current-context 2>/dev/null || echo '(none)')"
fi
echo

exec cargo run --features connector --no-default-features --bin ks-connector
