# Dependency advisory tracking

Companion to [`deny.toml`](../deny.toml). Each section explains why an
advisory is currently ignored, the upstream change that would let us
remove the ignore, and how to test that the ignore is still needed.

Re-run `cargo deny check advisories` whenever bumping `dioxus`,
`dioxus-desktop`, `kube`, `k8s-openapi`, `wry`, or `tao` and prune
entries below that no longer reproduce.

## GTK3 binding chain (RUSTSEC-2024-0411..0420)

| Crate              | Advisory          |
| ------------------ | ----------------- |
| `gdkwayland-sys`   | RUSTSEC-2024-0411 |
| `gdk`              | RUSTSEC-2024-0412 |
| `atk`              | RUSTSEC-2024-0413 |
| `gdkx11-sys`       | RUSTSEC-2024-0414 |
| `gtk`              | RUSTSEC-2024-0415 |
| `atk-sys`          | RUSTSEC-2024-0416 |
| `gdkx11`           | RUSTSEC-2024-0417 |
| `gdk-sys`          | RUSTSEC-2024-0418 |
| `gtk3-macros`      | RUSTSEC-2024-0419 |
| `gtk-sys`          | RUSTSEC-2024-0420 |

**Path:** `dioxus-desktop 0.6` → `wry 0.45` / `tao 0.30` → `gtk 0.18.x`.

**Why accepted:** the gtk-rs team marked the entire GTK3 family
unmaintained in favour of GTK4 bindings on 2024-11-26. No security
impact (CVE) is called out by the maintainers. Linux is the only target
affected; macOS/Windows builds do not pull the GTK chain.

**Removal trigger (any one of):**

- `dioxus-desktop` adopts a GTK4-backed `wry`/`tao`, or
- We migrate to `dioxus-native` (Blitz renderer) once it reaches parity
  with our current UI surface, or
- We switch desktop renderer entirely.

**Upstream status to watch:**

- Dioxus releases: <https://github.com/DioxusLabs/dioxus/releases>
- wry GTK4 tracking: <https://github.com/tauri-apps/wry/issues> (search
  "GTK4")

## proc-macro-error (RUSTSEC-2024-0370)

Transitive via `glib-macros 0.18` and `gtk3-macros 0.18`, i.e. the same
GTK3 chain above. Build-time only; no runtime exposure. Will drop out
automatically when the GTK3 chain is replaced.

## instant (RUSTSEC-2024-0384)

Transitive via two paths:

- `backoff 0.4` (kube-runtime 0.98)
- `tao 0.30` (dioxus-desktop)

Drops out together with the `backoff` advisory below (kube upgrade) and
the GTK3/tao chain.

## backoff (RUSTSEC-2025-0012)

Transitive via `kube-runtime 0.98`.

**Resolution path:** `kube-runtime 2.0+` replaced `backoff` with
`backon`. Upgrading `kube` to 2.x (plus `k8s-openapi` to 0.26, MSRV
1.85) clears the advisory.

**Why not done in this PR:** the upgrade touches ~39 files that consume
`kube`/`k8s_openapi` types and warrants a focused PR with its own
regression validation (watcher streams, Client config, RBAC paths).
Tracked as follow-up work.

## fxhash (RUSTSEC-2025-0057)

Transitive via `wry 0.45` → `kuchikiki 0.8` → `selectors 0.22`. Drops
out with the dioxus-desktop / wry upgrade tracked alongside the GTK3
chain.

## rustls-pemfile (RUSTSEC-2025-0134)

Transitive via `kube-client 0.98`. Repo archived 2025-08; maintainers
recommend `rustls-pki-types ≥ 1.9` directly.

**Resolution path:** same kube 2.x upgrade tracked under `backoff`
above; later kube-client revisions stopped pulling the wrapper.
