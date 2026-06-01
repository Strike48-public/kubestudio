# Dependency advisory tracking

Companion to [`deny.toml`](../deny.toml). Documents the upstream blocker
behind each currently-ignored advisory and how to test that the ignore
is still needed.

Re-run `cargo deny check` and `cargo audit` whenever bumping `dioxus`,
`dioxus-desktop`, `kube`, `k8s-openapi`, `wry`, `tao`, or the
`strike48-connector` / `strike48-proto` SDK pair and prune entries
below that no longer reproduce.

## Audit summary (issue #8)

| Advisory | Crate | Status |
| --- | --- | --- |
| RUSTSEC-2025-0012 | `backoff` | **Resolved** — kube-runtime 2.0 replaced with `backon` |
| RUSTSEC-2024-0384 | `instant` | **Resolved** — tao 0.34 + backon dropped it |
| RUSTSEC-2026-0002 | `lru` | **Resolved** — dioxus-isrg 0.7 uses a fixed lru |
| RUSTSEC-2024-0417 | `gdkx11` | **Resolved** — wry 0.53 doesn't enable the gdkx11 feature |
| RUSTSEC-2026-0097 (`rand` 0.7 phf chain) | `rand` | **Partial** — 0.7/phf-0.10 path cleared; 0.7 still reached via wry kuchikiki fork (see below) |
| RUSTSEC-2026-0097 (`rand` 0.8 ashpd/backoff chain) | `rand` | **Resolved** — rfd 0.17 / kube 2.0 / dioxus 0.7 dropped the paths |
| RUSTSEC-2026-0097 (`rand` 0.9 tokio-tungstenite/ashpd 0.11) | `rand` | **Resolved** — tokio-tungstenite 0.29 / rfd 0.17 dropped the paths |
| RUSTSEC-2023-0071 (`rsa` timing) | `rsa` | **Not applicable** — `rsa` is not in our lockfile |
| RUSTSEC-2024-0411..0420 (10 GTK3 bindings — 0417 cleared) | gtk-rs 0.18 family | **Upstream-blocked** (wry GTK4 migration) |
| RUSTSEC-2024-0370 | `proc-macro-error` | **Upstream-blocked** (drops with GTK3) |
| RUSTSEC-2024-0429 | `glib` 0.18 unsound | **Upstream-blocked** (drops with GTK3) |
| RUSTSEC-2025-0057 | `fxhash` | **Upstream-blocked** (wry kuchikiki fork) |
| RUSTSEC-2026-0097 (`rand` 0.7 — remaining) | `rand` | **Upstream-blocked** (wry kuchikiki fork) |
| RUSTSEC-2024-0436 | `paste` | **Upstream-blocked** (dioxus-desktop pulls `image` with default features → ravif → rav1e → paste) |
| RUSTSEC-2025-0134 | `rustls-pemfile` | **Upstream-blocked** (Strike48 SDK still uses tonic 0.12) |

7 advisories cleared by version bumps. 12 remain, all upstream-blocked
on concrete external work tracked below.

## GTK3 binding chain (RUSTSEC-2024-0411..0420 less 0417, plus 0370, 0429)

| Crate              | Advisory          |
| ------------------ | ----------------- |
| `gdkwayland-sys`   | RUSTSEC-2024-0411 |
| `gdk`              | RUSTSEC-2024-0412 |
| `atk`              | RUSTSEC-2024-0413 |
| `gdkx11-sys`       | RUSTSEC-2024-0414 |
| `gtk`              | RUSTSEC-2024-0415 |
| `atk-sys`          | RUSTSEC-2024-0416 |
| `gdk-sys`          | RUSTSEC-2024-0418 |
| `gtk3-macros`      | RUSTSEC-2024-0419 |
| `gtk-sys`          | RUSTSEC-2024-0420 |
| `proc-macro-error` | RUSTSEC-2024-0370 |
| `glib` (0.18)      | RUSTSEC-2024-0429 |

**Path:** `dioxus-desktop 0.7` → `wry 0.53` / `tao 0.34` → `gtk 0.18.x`.

**Why accepted:** the gtk-rs team marked the GTK3 family unmaintained on
2024-11-26 in favour of GTK4 bindings. No CVE called out. Linux is the
only target affected; macOS/Windows builds do not pull GTK.

**Removal trigger (any one of):**

- `dioxus-desktop` adopts a GTK4-backed `wry`/`tao`, or
- We migrate to `dioxus-native` (Blitz renderer) once it reaches parity
  with our current UI surface, or
- We switch desktop renderer entirely.

**Upstream status to watch:**

- wry GTK4 tracking: <https://github.com/tauri-apps/wry/issues/1474>
  (open, no merged PR as of wry 0.55.1)
- Dioxus releases: <https://github.com/DioxusLabs/dioxus/releases>

## fxhash + rand 0.7 (RUSTSEC-2025-0057, RUSTSEC-2026-0097 — partial)

Both come through `wry 0.53` → `kuchikiki 0.8.8-speedreader` (maintained
fork of kuchikiki) → `selectors 0.24` → `phf 0.8` → `fxhash` + `rand
0.7`. Drops out when wry retires the kuchikiki dependency, tracked
alongside the GTK4 migration linked above.

## paste (RUSTSEC-2024-0436)

`dioxus-desktop 0.7.9` declares `image = "0.25.6"` with default
features. The avif feature pulls `ravif 0.12` → `rav1e 0.8` →
`libfuzzer-sys` → `paste 1.0`. We cannot disable image default
features from this side without a `[patch.crates-io]` fork of
`dioxus-desktop`. Drops out when either:

- `dioxus-desktop` disables image default features, or
- `ravif` / `rav1e` move off `paste`.

The transitive `libfuzzer-sys 0.4.12` license (NCSA) is added to the
allow-list in `deny.toml` — OSI-approved permissive license,
comparable to MIT/BSD.

## rustls-pemfile (RUSTSEC-2025-0134)

`strike48-connector 0.4.1` → `tonic 0.12` → `rustls-pemfile`. `kube-client
2.0` no longer pulls `rustls-pemfile` (uses `pem` directly). The
remaining instance is gated on the Strike48 SDK upgrading to `tonic
0.13+`, which depends on `rustls-pki-types` directly.

## Out of scope today (not version-bump fixes)

Two alternatives have been considered and deferred:

1. **Drop the Linux desktop release artifact** — would eliminate every
   `target_os = "linux"` GTK3 + fxhash + paste advisory, but breaks
   the `install.sh` Linux native binary path documented in the
   README. Product decision, not an audit decision.
2. **`[patch.crates-io]` fork of `wry`/`tao`/`muda` onto gtk4-rs** —
   the upstream GTK4 migration is itself incomplete, so this would
   mean carrying our own port. Substantial ongoing maintenance.
