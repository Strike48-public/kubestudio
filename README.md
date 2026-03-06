# KubeStudio

A modern, high-performance Kubernetes desktop client built with Rust and Dioxus. Combines the speed of K9s with the visual polish of Lens.

## Features

**Cluster Management**
- Automatic kubeconfig detection with multi-context support
- Real-time cluster connection with live watch API
- Seamless context switching between clusters

**Resource Browser**
- 20+ Kubernetes resource types (Pods, Deployments, Services, ConfigMaps, Secrets, RBAC, etc.)
- Custom Resource Definitions (CRDs) with automatic discovery
- Live resource updates via Kubernetes watch API
- YAML/Describe view with syntax highlighting
- Create, edit, and delete resources with confirmation

**Workload Operations**
- Pod logs with streaming, multi-container selection, and follow mode
- Pod exec/shell with in-app terminal
- Port forwarding with in-app management
- Deployment scaling and rollout restart
- Job/CronJob triggering
- Node metrics from metrics-server (CPU/memory usage)

**Keyboard-Driven UX**
- K9s-style direct shortcuts (no menus required)
- Command palette (`Ctrl+K`) with external tool integration
- Command mode (`:` prefix) with resource aliases
- Vim-style navigation (j/k)
- Context-aware hotkeys bar

**AI Agent Chat** (optional, requires [Matrix](https://github.com/Strike48/matrix) backend)
- Right-side slide-out chat panel with resizable width
- Matrix-backed AI agents with tool-calling (cluster inspection, kubectl via toolbox)
- Rich rendering: markdown, collapsible tool calls, thinking blocks
- Per-agent conversation history with create/switch/delete
- "Ask Agent" contextual buttons on cluster warning events
- "Generate Report" one-click cluster summary via sidebar
- Auto-scroll with floating scroll-to-bottom indicator

**Plugin System**
- Resource aliases (`:dp` → Deployments, `:po` → Pods)
- Custom hotkey bindings
- External tool launchers (k9s, stern, kubectl) via command palette
- Configuration at `~/.config/kubestudio/config.yaml`

**Performance**
- Native Rust binary - instant startup
- Low memory footprint (~100MB)
- Virtual scrolling for large resource lists
- Async throughout with Tokio

## Running

### Prerequisites

- **Rust toolchain** — for building from source
- **Kubernetes cluster** — local (minikube, kind, colima) or cloud (EKS, GKE, AKS)
- **kubeconfig** — valid configuration at `~/.kube/config`

### Desktop (Standalone)

Run the native desktop app. Reads your local kubeconfig and connects directly to clusters:

```bash
RUST_LOG=info cargo run
```

### Connector Mode (Local Dev)

Run as a Matrix connector with AI chat enabled. Requires a [Matrix](https://github.com/Strike48/matrix) backend:

```bash
RUST_LOG=info \
KUBESTUDIO_AI=true \
KUBESTUDIO_MODE=write \
STRIKE48_URL=grpc://connectors-studio.strike48.test:80 \
STRIKE48_API_URL=https://studio.strike48.test \
TENANT_ID=non-prod \
INSTANCE_ID=kubestudio-local \
MATRIX_TLS_INSECURE=true \
cargo run --features connector --no-default-features --bin ks-connector
```

| Variable | Description |
|----------|-------------|
| `KUBESTUDIO_AI` | Enable AI chat panel (`true` / `false`) |
| `KUBESTUDIO_MODE` | Permission mode: `read` (view-only) or `write` (full access) |
| `STRIKE48_URL` | Strike48 Prospector Studio gRPC endpoint |
| `STRIKE48_API_URL` | Strike48 Prospector Studio API base URL |
| `TENANT_ID` | Tenant identifier |
| `INSTANCE_ID` | Connector instance name |
| `MATRIX_TLS_INSECURE` | Skip TLS verification (dev only) |

### Docker

The image builds both `ks-server` (standalone) and `ks-connector` (AI-enabled) binaries. Defaults to `ks-server`.

Pre-built images are published to GHCR on every release:

```bash
docker run -p 8080:8080 \
  -v ~/.kube/config:/etc/kubestudio/kubeconfigs/default:ro \
  -e KUBECONFIG=/etc/kubestudio/kubeconfigs/default \
  ghcr.io/strike48/kubestudio:latest
```

Or build locally:

```bash
docker build -t kubestudio .
docker run -p 8080:8080 \
  -v ~/.kube/config:/etc/kubestudio/kubeconfigs/default:ro \
  -e KUBECONFIG=/etc/kubestudio/kubeconfigs/default \
  kubestudio
```

### Helm (In-Cluster)

Deploy into a Kubernetes cluster. Two modes controlled by the `mode` value:

**Standalone** — web UI only, no external dependencies:

```bash
helm install kubestudio chart/kubestudio \
  --set rbac.clusterRole=cluster-admin
```

Optionally mount a kubeconfig (cluster/context names carry over):

```bash
helm install kubestudio chart/kubestudio \
  --set-file kubeconfigs.default=~/.kube/config
```

**AI-enabled** — web UI + Matrix connector for AI chat:

```bash
helm install kubestudio chart/kubestudio \
  --set mode=ai-enabled \
  --set kubestudio.permissionMode=write \
  --set connector.strikeApiUrl=https://your-strike48-instance \
  --set connector.strikeUrl=grpc://your-matrix-host:50061 \
  --set connector.tenantId=your-tenant \
  --set connector.instanceId=kubestudio \
  --set rbac.clusterRole=cluster-admin
```

See `chart/kubestudio/values.yaml` for all configuration options.

## Keyboard Shortcuts

Press keys directly - no menu required.

### Navigation

| Key | Action |
|-----|--------|
| `o` / `0` | Cluster Overview |
| `p` / `1` | Pods |
| `2` | Deployments |
| `3` | Services |
| `4` | ConfigMaps |
| `5` | Secrets |
| `N` | Nodes |
| `v` | Events |
| `Shift+F` | Port Forwards list |
| `/` | Focus search |
| `Esc` | Go back / Clear search |
| `Ctrl+K` | Command palette |
| `Ctrl+B` | Toggle sidebar |
| `Shift+C` | Toggle AI chat panel |
| `:` | Command mode |
| `?` | Help |

### Command Mode

Press `:` then type an alias:

| Alias | Resource |
|-------|----------|
| `po` | Pods |
| `dp` | Deployments |
| `svc` | Services |
| `cm` | ConfigMaps |
| `sec` | Secrets |
| `no` | Nodes |

### Resource Actions

| Key | Action |
|-----|--------|
| `Enter` | Select / Drill down |
| `y` | View YAML |
| `d` | Describe resource |
| `l` | View logs (pods) |
| `s` | Shell/exec (pods) |
| `f` | Port forward (pods) |
| `e` | Edit resource |
| `D` | Delete resource |
| `R` | Restart (deployments/statefulsets) |
| `T` | Trigger (cronjobs) |
| `+` / `-` | Scale up/down |
| `Ctrl+O` | Apply manifest from file |
| `Ctrl+N` | Create new resource |

### Viewer Controls

| Key | Action |
|-----|--------|
| `w` | Toggle text wrap |
| `r` | Reveal/mask secrets |
| `c` | Copy to clipboard |
| `j` / `k` | Navigate up/down |
| `Tab` | Switch focus zones |

## Command Palette

Press `Ctrl+K` to open the command palette. Available commands include:

- **k9s** - Open k9s in current namespace
- **stern** - Tail logs with stern (requires stern installed)
- **kubectl** - Run kubectl commands
- **External terminal** - Open terminal with kubeconfig context set

## Configuration

Plugin configuration lives at `~/.config/kubestudio/config.yaml`:

```yaml
aliases:
  dp: deployments
  po: pods
  svc: services

hotkeys:
  - key: "Ctrl+Shift+L"
    command: "stern {{name}} -n {{namespace}}"
    description: "Tail logs with stern"
    requires_selection: true
    open_terminal: true

tools:
  - name: k9s
    command: k9s
    args: ["--context", "{{context}}", "--namespace", "{{namespace}}"]
```

Template variables: `{{namespace}}`, `{{name}}`, `{{kind}}`, `{{context}}`

## Architecture

```
studio-kube-desktop/
├── crates/
│   ├── ks-core/        # Core types and error handling
│   ├── ks-kube/        # Kubernetes client, toolbox, Matrix chat client
│   ├── ks-plugin/      # Plugin and configuration system
│   ├── ks-state/       # Application state management
│   └── ks-ui/          # Dioxus UI, servers, and connectors
│       └── src/bin/
│           ├── ks-server.rs        # Standalone web server (liveview)
│           ├── ks-connector.rs     # Matrix connector (UI + AI tools)
│           └── ks-tool-connector.rs # Tool-only Matrix connector
├── chart/kubestudio/   # Helm chart for in-cluster deployment
└── Dockerfile          # Multi-stage build (ks-server + ks-connector)
```

| Binary | Feature Flag | Description |
|--------|-------------|-------------|
| `kubestudio` | `desktop` (default) | Native desktop app |
| `ks-server` | `server` | Standalone web UI via Dioxus liveview |
| `ks-connector` | `connector` | Web UI + Matrix AI connector with cluster tools |
| `ks-tool-connector` | `connector` | Lightweight tool-only connector for AI agents |

### Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 2024 edition |
| UI Framework | Dioxus 0.6 |
| K8s Client | kube-rs 0.98 |
| Async Runtime | Tokio |
| Styling | CSS (Catppuccin-inspired dark theme) |

## Development

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test --workspace
```

See `PRD.md` for detailed implementation status and roadmap.

## Comparison

| Feature | Lens | K9s | KubeStudio |
|---------|------|-----|------------|
| Platform | Electron | Terminal | Native (Rust) |
| Memory | ~500MB | ~30MB | ~100MB |
| Startup | 3-5s | <1s | <500ms |
| Visual UI | Yes | No | Yes |
| Keyboard-first | Partial | Yes | Yes |
| CRD Support | Yes | Yes | Yes |
| Plugin System | Yes | Limited | Yes |
| Open source | Partial | Yes | Yes (MPL-2.0) |

## Roadmap

### Completed

- **v0.1-0.4** - Core browser, YAML editing, watch API, pod exec, port forwarding
- **v0.5** - CRD support, virtual scrolling, metrics-server integration
- **v0.6** - Plugin system, command mode, external tool launchers, AI agent chat panel

### Planned

- **v0.7** - Multi-cluster aggregation, light theme
- **v1.0** - Production release, installers, documentation

## License

This project is licensed under the **Mozilla Public License 2.0** (MPL-2.0). See [LICENSE](LICENSE) for the full text.

- You are free to use, modify, and distribute this software
- Modifications to MPL-licensed files must be shared under MPL-2.0
- MPL-2.0 is compatible with integration into larger works under other licenses

For the full Strike48 platform Terms of Service, see [strike48.com/terms-of-service](https://www.strike48.com/terms-of-service).

---

**Built with Rust and Dioxus**
