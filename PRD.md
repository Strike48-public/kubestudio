# Product Requirements Document: KubeStudio

**Version:** 1.1
**Date:** February 10, 2026
**Author:** Strike48 Team

---

## 1. Executive Summary

**KubeStudio** is a modern, high-performance desktop application for viewing and managing Kubernetes clusters. Built with Rust and Dioxus, it combines the power of native performance with a reactive UI to deliver a Lens/K9s-like experience with improved speed, lower resource usage, and cross-platform support.

---

## 2. Problem Statement

### Current Pain Points
- **Lens** is feature-rich but resource-heavy (Electron-based), consuming significant RAM/CPU
- **K9s** is fast and terminal-native but lacks visual appeal and accessibility for less CLI-savvy users
- Existing tools often lag behind Kubernetes API updates
- Multi-cluster management is cumbersome in most tools
- No unified solution offers both performance and modern UX

### Opportunity
Build a native desktop app that combines:
- K9s-level performance and keyboard-driven workflows
- Lens-level visual polish and discoverability
- Modern Rust ecosystem benefits (safety, speed, small binary)

---

## 3. Target Users

| Persona | Description | Key Needs |
|---------|-------------|-----------|
| **Platform Engineer** | Manages multiple production clusters | Multi-cluster view, RBAC awareness, audit logs |
| **Developer** | Deploys and debugs applications | Pod logs, exec into containers, port-forwarding |
| **SRE/Ops** | Monitors cluster health, responds to incidents | Resource metrics, alerts, quick actions |
| **DevOps Learner** | Learning Kubernetes concepts | Intuitive UI, resource relationships, inline help |

---

## 4. Core Features

### 4.1 Cluster Management
- [x] Connect via kubeconfig (multi-context support)
- [x] Add/remove/switch clusters
- [x] Cluster overview dashboard (o/0 hotkey)
- [x] Namespace filtering and favorites

### 4.2 Resource Browser
- [x] Hierarchical view of all K8s resources
- [x] Real-time updates via watch API (Live indicator in status bar)
- [x] YAML/JSON view with syntax highlighting
- [x] Create, edit, delete resources (with confirmation modal)
- [x] Apply manifests from file (Ctrl+O)

### 4.3 Workload Management
- [x] Pods: logs with multi-container drill-down, exec shell (s hotkey), port-forward (f hotkey)
- [x] Deployments: scale (+/- hotkeys), rollout restart (R hotkey), rollout status (planned), rollback (planned)
- [x] Jobs/CronJobs: trigger (T hotkey), view history (planned)
- [x] StatefulSets, DaemonSets, ReplicaSets support (R hotkey for restart)

### 4.4 Networking & Storage
- [x] Services: endpoints, port mappings
- [x] Ingress/Gateway API visualization
- [x] ConfigMaps and Secrets (secrets masked by default, r to reveal)
- [x] PVs/PVCs with storage class info

### 4.5 Cluster Administration
- [x] Node overview with resource usage (N hotkey, CPU/memory columns)
- [x] RBAC viewer (Roles, ClusterRoles, Bindings in sidebar)
- [x] Events view with filtering (v hotkey)
- [x] Custom Resource Definitions (CRDs) - see Section 4.7

### 4.6 User Experience
- [x] Keyboard-first navigation (vim-style optional)
- [x] Command palette (Ctrl+I)
- [x] Customizable themes (dark/light - dark implemented)
- [ ] Split panes for multi-resource view
- [x] Search/filter across all resources
- [ ] Bookmarks and quick actions

### 4.7 Custom Resource Definitions (CRDs)

**Foundation (Implemented):**
- [x] DynamicObject support for arbitrary resource types
- [x] Generic YAML apply works with any apiVersion/kind

**Discovery & Viewing (Implemented):**
- [x] List CRDs in cluster via watch API
- [x] Dynamic sidebar entries under "Custom Resources" category
- [x] CRD resource listing with status extraction from printer columns
- [x] Watch support for custom resources (real-time updates)
- [x] Describe/YAML view for custom resources
- [x] Delete support for custom resources
- [ ] CRD schema viewer (show validation rules)

**Common CRDs to Support:**
- ArgoCD: Application, AppProject
- Cert-Manager: Certificate, Issuer, ClusterIssuer
- Istio: VirtualService, Gateway, DestinationRule
- Prometheus: ServiceMonitor, PrometheusRule, PodMonitor
- Crossplane: Composite resources

### 4.8 Plugin & Extension System

**K9s-style Features:**
- [ ] Custom resource columns via JSONPath expressions
- [x] Resource aliases via command mode (`:` then alias, e.g., `:dp` → Deployments)
- [x] Custom hotkey → shell command bindings
- [ ] Skin/theme customization beyond presets

**External Tool Integration:**
- [x] Launch external terminal with kubeconfig context set
- [x] Custom plugin commands via command palette (Ctrl+I)
- [x] Built-in echo-test plugin for pipeline validation
- [x] echo-test and kubectl-get as built-in default plugins

**Extension Points:**
- [x] Plugin manifest format (YAML config at `~/.config/kubestudio/config.yaml`)
- [ ] Custom views for specific resource types
- [ ] Plugin discovery and installation

**Architecture Decision:** YAML-based configuration (simple, no runtime needed)

### 4.9 In-Cluster Deployment

KubeStudio can be deployed inside a Kubernetes cluster as a web application using Docker and Helm.

**Deployment Modes:**
- **Standalone** (`ks-server`) — Web UI only via Dioxus liveview, no external dependencies
- **AI-enabled** (`ks-connector`) — Web UI + Matrix connector for AI chat features

**Container Image:**
- Multi-stage build with `cargo-chef` for dependency caching
- `debian:bookworm-slim` runtime (no GTK/WebKit — liveview renders in browser)
- Non-root user (UID 999), read-only root filesystem
- Entrypoint via `tini` for proper signal handling

**Helm Chart (`chart/kubestudio/`):**
- Configurable `mode` (standalone / ai-enabled)
- Kubeconfig volume mounts via Secret (inline values or existing Secret)
- RBAC with configurable ClusterRole (view, edit, admin, cluster-admin)
- Optional Ingress with TLS support
- Health/readiness/startup probes
- PodDisruptionBudget support

**Authentication:** External only (ingress-level oauth2-proxy or VPN). No built-in auth in the server.

---

## 5. Technical Architecture

### 5.1 Technology Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| **Language** | Rust (2024 edition) | Memory safety, performance, cross-platform |
| **UI Framework** | Dioxus 0.6+ | Native performance, React-like DX, hot reload |
| **K8s Client** | kube-rs | Official Rust client, async, well-maintained |
| **Async Runtime** | Tokio | Industry standard for Rust async |
| **Serialization** | serde + serde_yaml/json | K8s manifest handling |
| **Storage** | SQLite (via rusqlite) | Local preferences, bookmarks, history |
| **Packaging** | Tauri or Dioxus Desktop | Native windowing, small binary |

### 5.2 Architecture Diagram

```
+------------------------------------------+
|               KubeStudio                 |
+------------------------------------------+
|                  UI Layer                |
|  (Dioxus Components, State Management)   |
+------------------------------------------+
|              Service Layer               |
|  (Cluster Manager, Resource Watchers)    |
+------------------------------------------+
|              Client Layer                |
|  (kube-rs, Auth, Connection Pool)        |
+------------------------------------------+
           |              |
    +------+------+  +----+----+
    | Cluster A   |  | Cluster B |
    | (K8s API)   |  | (K8s API) |
    +-------------+  +-----------+
```

### 5.3 Key Design Decisions

1. **Async-first**: All K8s API calls are non-blocking
2. **Watch-based updates**: Use K8s watch API for real-time resource sync
3. **Local caching**: Cache resource state for instant UI, sync in background
4. **Modular client architecture**: Operations split by resource category (workloads, networking, storage, etc.)
5. **DynamicObject foundation**: Generic resource handling enables CRD support without hardcoded types
6. **Generation-based cluster switching**: Watchers use generation counters to handle stale events during context switches

---

## 6. Non-Functional Requirements

### 6.1 Performance
- App startup: < 500ms cold, < 200ms warm
- Memory usage: < 150MB baseline, < 500MB with 10 clusters
- CPU idle: < 1% when not actively watching resources
- Binary size: < 30MB

### 6.2 Platform Support
- macOS (Apple Silicon + Intel)
- Linux (x86_64, ARM64)
- Windows 10/11 (x86_64)

### 6.3 Security
- No credentials stored in plaintext
- Support for all kubeconfig auth methods (exec, OIDC, certificates)
- Secret values masked by default, reveal on explicit action
- No telemetry without explicit opt-in

### 6.4 Accessibility
- Full keyboard navigation
- Screen reader support
- High contrast theme option
- Configurable font sizes

---

## 7. MVP Scope (v0.1)

### In Scope
- Single cluster connection via kubeconfig
- Browse core resources: Pods, Deployments, Services, ConfigMaps, Secrets
- View pod logs (single container)
- YAML viewer for any resource
- Delete resources
- Basic search/filter
- Dark theme

### Out of Scope (Post-MVP)
- Multi-cluster management
- Pod exec/shell
- Port forwarding
- Helm/Kustomize integration
- Resource editing
- Metrics/monitoring integration
- Plugin system

---

## 8. Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Startup time | < 500ms | Automated benchmark |
| Memory (idle) | < 100MB | Profiling |
| User satisfaction | > 4.5/5 | Beta feedback survey |
| GitHub stars (3mo) | > 500 | GitHub |
| Active users (6mo) | > 1,000 | Opt-in telemetry |

---

## 9. Milestones

| Phase | Deliverable |
|-------|-------------|
| **M1: Foundation** | Project setup, kube-rs integration, basic cluster connection |
| **M2: Core UI** | Resource browser, navigation, YAML viewer |
| **M3: Workloads** | Pod logs, deployments, services |
| **M4: Polish** | Search, themes, keyboard shortcuts |
| **M5: MVP Release** | v0.1.0 public release |
| **M6: Multi-cluster** | Multiple cluster support, context switching |
| **M7: Advanced** | Exec, port-forward, resource editing |

---

## 10. Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Dioxus breaking changes | High | Medium | Pin versions, contribute upstream |
| kube-rs auth edge cases | Medium | Medium | Extensive testing with various providers |
| Cross-platform UI inconsistencies | Medium | High | CI testing on all platforms |
| Performance with large clusters | High | Medium | Pagination, virtual scrolling, lazy loading |

---

## 11. Open Questions

1. ~~Should we support Helm chart management in MVP?~~ Deferred to post-v1.0
2. ~~Plugin system architecture - Lua, WASM, or native Rust?~~ ✅ YAML-based config (v0.6)
3. ~~Bundled terminal emulator or rely on system terminal for exec?~~ ✅ In-app terminal implemented
4. Pricing model if we add premium features later?
5. CRD column detection - use printer columns from CRD spec or allow user customization?
6. Plugin sandboxing - how to safely execute user-provided plugins?

---

## 12. References

- [Dioxus Documentation](https://dioxuslabs.com/docs/0.6/)
- [kube-rs](https://kube.rs/)
- [Kubernetes API Reference](https://kubernetes.io/docs/reference/kubernetes-api/)
- [Lens](https://k8slens.dev/) - Competitor analysis
- [K9s](https://k9scli.io/) - Competitor analysis

---

## Appendix A: Implementation Status & Roadmap

### A.1 Current Architecture

**Crate Structure:**
```
kubestudio/
├── crates/
│   ├── ks-core/              # Core types and error handling
│   ├── ks-kube/              # Kubernetes API client
│   │   └── src/client/       # Modular operations (12 modules)
│   │       ├── mod.rs        # KubeClient struct
│   │       ├── types.rs      # WatchEvent, handles
│   │       ├── workloads.rs  # Pod, Deployment, StatefulSet, etc.
│   │       ├── config.rs     # ConfigMap, Secret
│   │       ├── networking.rs # Service, Endpoints, Ingress
│   │       ├── storage.rs    # PV, PVC, StorageClass
│   │       ├── rbac.rs       # Role, ClusterRole, Bindings
│   │       ├── cluster.rs    # Namespace, Node, Event
│   │       ├── watch.rs      # Real-time watch streams
│   │       ├── drilldown.rs  # Related resource lookups
│   │       ├── apply.rs      # DynamicObject YAML apply
│   │       ├── exec.rs       # Pod exec, port-forward
│   │       ├── crd.rs        # CRD discovery and dynamic ops
│   │       └── yaml.rs       # YAML serialization
│   ├── ks-plugin/            # Plugin and configuration system
│   │   └── src/
│   │       ├── lib.rs        # Config loading
│   │       ├── config.rs     # PluginConfig, Alias, Hotkey, Tool types
│   │       └── executor.rs   # Command execution
│   ├── ks-state/             # Application state management
│   └── ks-ui/                # Dioxus desktop UI
│       └── src/
│           ├── app/          # Main app (5 modules)
│           │   ├── mod.rs    # Main component, keyboard handler
│           │   ├── helpers.rs
│           │   ├── hotkeys.rs
│           │   ├── crd_items.rs
│           │   └── resource_items.rs
│           ├── components/
│           │   ├── yaml_viewer/  # YAML editor (5 modules)
│           │   └── ...           # Other components
│           └── hooks/            # React-style hooks
└── Cargo.toml                # Workspace root
```

**Key Components:**
- **ks-core**: `SkdError`, `SkdResult` types for consistent error handling across crates
- **ks-kube**: `KubeClient` with modular operations by resource category
- **ks-plugin**: Plugin configuration, aliases, custom hotkeys, external tools
- **ks-state**: `Store`, `AppState`, `View` for reactive state management
- **ks-ui**: Dioxus components, hooks, and keyboard-driven interface

**Plugin Configuration:**
The plugin system uses a YAML config file:
- macOS: `~/Library/Application Support/kubestudio/config.yaml`
- Linux: `~/.config/kubestudio/config.yaml`

```yaml
# Resource aliases (use with command mode: press ":" then type alias)
# Example: ":dp" navigates to deployments, ":sec" to secrets
aliases:
  dp: deployments
  po: pods
  svc: services
  sec: secrets
  cm: configmaps

# Custom hotkey bindings
hotkeys:
  - key: "Ctrl+Shift+K"
    command: "kubectl describe {{kind}} {{name}} -n {{namespace}} --context {{context}}"
    description: "Describe resource with kubectl"
    requires_selection: true
    open_terminal: true

# External tools (available in command palette via Ctrl+I)
# Built-in defaults: echo-test, kubectl-get
# Users can add their own tools here
tools:
  - name: my-tool
    command: my-tool
    args: ["--namespace", "{{namespace}}"]
    description: "My custom tool"
```

Template variables: `{{namespace}}`, `{{name}}`, `{{kind}}`, `{{context}}`

### A.2 MVP v0.1 Status (Current)

#### ✅ Completed Features

**Cluster & Context Management:**
- Kubeconfig loading and parsing
- Multi-context switching
- Connection status indicators
- Namespace filtering (all namespaces or specific namespace)

**Resource Browser (21 resource types):**
- **Workloads**: Pod, Deployment, StatefulSet, DaemonSet, ReplicaSet, Job, CronJob
- **Configuration**: ConfigMap, Secret
- **Networking**: Service, Endpoints, Ingress
- **Storage**: PersistentVolume, PersistentVolumeClaim, StorageClass
- **RBAC**: Role, ClusterRole, RoleBinding, ClusterRoleBinding
- **Cluster**: Node, Namespace, Event

**Operations:**
- List resources (namespace-scoped or cluster-wide)
- Delete resources with confirmation modal (keyboard navigable)
- View pod logs with wrap toggle and smooth scrolling
- Multi-container pod drill-down for log selection
- Search/filter resources by name
- YAML/Describe view with syntax highlighting

**UI/UX:**
- Dark theme (Catppuccin-inspired)
- Sidebar with collapsible resource categories and keyboard navigation
- Resizable sidebar with constraints
- Command palette (Ctrl+I)
- Status bar with cluster/namespace info
- Context-aware hotkeys bar
- Keyboard shortcuts (k9s-style):
  - `o`/`0`: Overview
  - `p`: Pods, `d`: Deployments, `s`: Services, `v`: Events
  - `/`: Focus search
  - `Esc`: Clear search and refocus app
  - `Enter`: Select/drill into resource
  - `l`: View logs (when pod selected)
  - `y`: View YAML/describe (when resource selected)
  - `w`: Toggle text wrap (in log/YAML views)
  - `r`: Reveal/hide secret values (in secret YAML view)
  - `Tab`: Switch focus between sidebar and content
  - `j/k` or `↑/↓`: Navigate lists

**Performance:**
- Native Rust binary
- Async/await throughout
- Reactive signals for UI updates

#### ❌ Not Yet Implemented (Post v0.5)

**Resource Operations:**
- Rollout status/rollback for Deployments

**Advanced Features:**
- ~~CRD discovery and viewing~~ ✅ Implemented in v0.5
- Plugin/extension system (see Section 4.8)
- ~~Resource metrics integration (metrics-server)~~ ✅ Implemented in v0.5
- ~~Virtual scrolling for large resource lists~~ ✅ Implemented in v0.5
- Split panes for multi-resource view
- Bookmarks and quick actions

### A.3 Roadmap

#### v0.2 - Resource Details & Editing ✅ COMPLETE
**Goal:** Complete CRUD operations and resource inspection

- ✅ Wire YamlViewer to resource selection
- ✅ Implement resource editing (YAML editor with server-side apply)
- ✅ Implement resource creation (13 templates + custom YAML)
- ✅ Add apply-from-file functionality (Ctrl+O)
- ✅ Add proper multi-container log selection
- ✅ Implement streaming pod logs (with follow mode and timestamps)
- ✅ Add basic validation on edits (via dry-run apply)

**Completed:** January 2026

#### v0.3 - Real-time & Advanced Operations (Q2 2026) ✅ COMPLETE
**Goal:** Live updates and operational capabilities

- ✅ Add Events view (v hotkey)
- ✅ Add secret value masking with reveal-on-click (r hotkey in YAML view)
- ✅ Integrate Kubernetes watch API for real-time resource updates (Live indicator in status bar)
- ✅ Implement pod exec/shell with in-app terminal (s hotkey, proper cursor/backspace handling)
- ✅ Implement in-app port forwarding (f hotkey for modal, Shift+F for active list, Ctrl+D to stop)
- ✅ Add deployment scaling (+/- hotkeys)

**Completed:** January 2026

#### v0.4 - Cluster Health & Monitoring ✅ COMPLETE
**Goal:** Observability and cluster administration

- ✅ Node overview with resource usage (N hotkey, CPU/memory columns)
- ✅ RBAC viewer (Roles, ClusterRoles, RoleBindings, ClusterRoleBindings in sidebar)
- ✅ Enhanced overview dashboard (o/0 hotkey - nodes, pods, workloads, warnings)

**Completed:** February 2026

#### v0.5 - CRD Support & Performance ✅ COMPLETE
**Goal:** Custom resource support and scalability

**CRD Discovery:** ✅ COMPLETE
- ✅ List CRDs in cluster via watch API
- ✅ Dynamic sidebar entries under "Custom Resources" category
- ✅ CRD resource listing with status extraction
- ✅ Watch support for custom resources
- ✅ Describe/YAML view and delete support

**Performance:** ✅ COMPLETE
- ✅ Virtual scrolling for large resource lists (1000+ items)
- ✅ Resource metrics integration (metrics-server with graceful degradation)
- Pagination for API responses (deferred - watch handles current scale)

#### v0.6 - Plugin System & Productivity ✅ PLUGIN SYSTEM COMPLETE
**Goal:** Extensibility and power-user features

**Plugin System:** ✅ COMPLETE
- ✅ Plugin manifest format (YAML config at `~/.config/kubestudio/config.yaml`)
- Custom resource columns via JSONPath (planned)
- ✅ Resource aliases (e.g., `dp` → Deployments, 20+ built-in aliases)
- ✅ Custom hotkey → shell command bindings
- ✅ External tool launchers via command palette (Ctrl+I)

**Productivity:** (Remaining)
- Split panes for side-by-side comparison
- Bookmarks and quick actions
- Recently viewed resources
- Favorite namespaces

#### v0.7 - Multi-cluster & Polish
**Goal:** Advanced multi-cluster workflows

- True multi-cluster view (aggregate resources across clusters)
- Resource relationship visualization
- Light theme
- Customizable keyboard shortcuts
- Settings persistence

#### v1.0 - Production Ready
**Goal:** Polish, stability, documentation

- Comprehensive documentation
- Cross-platform installers (DMG, AppImage, MSI)
- Performance benchmarking and optimization
- Security audit
- Accessibility improvements (screen reader support)
- CI/CD for releases
- Website and marketing materials

### A.4 Known Technical Debt

**Current Issues:**
1. ~~**Polling vs Watch**: Resources refresh on-demand, not via Kubernetes watch streams~~ ✅ Fixed in v0.3
2. ~~**No pagination**: Large clusters (1000+ resources) may cause UI lag~~ Mitigated with virtual scrolling in v0.5
3. ~~**Static logs**: Pod logs are read once, not streamed in real-time~~ ✅ Fixed in v0.2
4. ~~**No virtual scrolling**: Resource lists render all items (impacts performance)~~ ✅ Fixed in v0.5
5. **Namespace selector edge case**: Empty string handling required sentinel value workaround

**Planned Refactors:**
- ~~Introduce resource cache with watch-based updates (v0.3)~~ ✅ Done
- ~~Add virtual scrolling to ResourceList component (v0.4)~~ ✅ Done in v0.5
- ~~Stream pod logs with proper backpressure handling (v0.3)~~ ✅ Done in v0.2
- ~~Implement proper form handling for resource creation/editing (v0.2)~~ ✅ Done

### A.5 Contributing

For contributors:
- See `CLAUDE.md` for session context and development notes
- Run `cargo run` to start the app (requires Kubernetes cluster access)
- Run `RUST_LOG=debug cargo run` for verbose logging

**Code structure:**
- Components: `crates/ks-ui/src/components/`
- Hooks: `crates/ks-ui/src/hooks/`
- Main app: `crates/ks-ui/src/app/mod.rs`
- K8s client: `crates/ks-kube/src/client/` (12 modules)
- State management: `crates/ks-state/src/store.rs`

---

## Appendix B: Competitive Analysis

| Feature | Lens | K9s | KubeStudio |
|---------|------|-----|---------------------|
| Platform | Electron | Terminal | Native (Rust) |
| Memory usage | ~500MB | ~30MB | Target: <150MB |
| Startup time | ~3-5s | <1s | Target: <500ms |
| Multi-cluster | Yes | Yes | Planned (v0.7) |
| CRD Support | Yes | Yes | ✅ Yes |
| Extensions | Yes | Limited | ✅ Yes |
| Visual UI | Yes | No | Yes |
| Keyboard-first | Partial | Yes | Yes |
| Real-time updates | Yes | Yes | ✅ Yes |
| Resource editing | Yes | No | ✅ Yes |
| Pod exec | Yes | Yes | ✅ Yes |
| Port forwarding | Yes | Yes | ✅ Yes |
| RBAC viewer | Yes | Yes | ✅ Yes |
| Node metrics | Yes | Yes | ✅ Yes |
| Open source | Partial | Yes | Yes (MPL-2.0) |
| Binary size | ~200MB | ~15MB | Target: <30MB |

**Positioning:**
- **vs Lens**: Lighter, faster, native performance without Electron overhead
- **vs K9s**: Visual interface, more accessible to non-CLI users, modern UX
- **Unique value**: Combines K9s speed with Lens polish in a truly native app
