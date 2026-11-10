# Running the KubeStudio connector

`ks-connector` runs KubeStudio as a Strike48 Studio connector: it registers with Studio
over gRPC, serves the KubeStudio UI inside the Studio portal, and gives Studio's AI agents
tools for the clusters in your kubeconfig.

## Prerequisites

- A kubeconfig (`$KUBECONFIG`, or `~/.kube/config`)
- A Rust toolchain (the script builds the connector with `cargo run`)
- Your Strike48 tenant ID
- Network access to `studio.strike48.com` and `connectors.studio.strike48.com:443`

## Quick start

```bash
TENANT_ID=my-org scripts/run-connector.sh
```

The script connects to `studio.strike48.com`, then builds and starts `ks-connector`. Once
it registers, KubeStudio appears in the Studio portal.

The connector starts in **read** mode (view-only). To allow changes to cluster
resources:

```bash
TENANT_ID=my-org KUBESTUDIO_MODE=write scripts/run-connector.sh
```

Other overrides:

```bash
INSTANCE_ID=my-kubestudio TENANT_ID=my-org scripts/run-connector.sh   # instance name
RUST_LOG=debug TENANT_ID=my-org scripts/run-connector.sh              # debug logging
```

## Local development

Against a local Strike48 stack on `*.strike48.test`:

```bash
scripts/run-connector.sh --local
```

This uses `studio.strike48.test`, tenant `non-prod` by default, and disables TLS
verification for the stack's self-signed certificates. It is the same configuration as the
README's "Connector Mode (Local Dev)" command.

## Environment variables

The script sets these; export any of them yourself to run `ks-connector` directly.

| Variable | Description | Script default |
|----------|-------------|----------------|
| `STRIKE48_API_URL` | Studio API base URL | `https://studio.strike48.com` |
| `STRIKE48_URL` | Studio connector gRPC endpoint | `grpc://connectors.studio.strike48.com:443` |
| `TENANT_ID` | Strike48 tenant | none (required); `non-prod` with `--local` |
| `INSTANCE_ID` | Connector instance name, unique per deployment | `kubestudio-<hostname>` |
| `CONNECTOR_NAME` | Gateway identity and sidebar label (see README) | unset |
| `KUBESTUDIO_MODE` | `read` (view-only) or `write` (full access) | `read` |
| `KUBESTUDIO_AI` | AI chat panel; `false` or `0` disables it | `true` |
| `MATRIX_TLS_INSECURE` | Skip TLS verification (dev only) | `false`; `true` with `--local` |
| `RUST_LOG` | Log level | `info` |

## How it fits together

```
Strike48 Studio (studio.strike48.com)
        ^
        | gRPC, port 443
        v
ks-connector ---- local socket ---- KubeStudio UI (Dioxus liveview)
        |
        | kubeconfig
        v
Kubernetes cluster(s)
```

The UI server listens on a local socket, not a TCP port: `/tmp/ks-connector-<pid>.sock` on
macOS and Linux, a named pipe on Windows. Studio reaches it through the connector, so open
KubeStudio from the Studio portal rather than a local URL.

## Demo data

`docs/demo-data.yaml` creates two sample namespaces with deployments, a stateful set, a
daemon set, jobs and cron jobs, services, config maps, secrets, a volume claim and RBAC, so
there is something to browse. Apply it to a disposable cluster only:

```bash
kubectl apply -f docs/demo-data.yaml
```

The secret values in it are placeholders.

## Troubleshooting

**The connector cannot reach Studio**

```bash
curl -I https://studio.strike48.com
nc -zv connectors.studio.strike48.com 443
RUST_LOG=debug TENANT_ID=my-org scripts/run-connector.sh
```

**Cluster calls fail**

```bash
kubectl config current-context
kubectl cluster-info
```

**"Permission denied" from the cluster:** check the RBAC permissions of the credentials in
your kubeconfig. In read mode, write actions are refused by design.
