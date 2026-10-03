# cargo deny & cargo audit

## cargo deny — **PASS** (warnings may still be present)

```
warning[wildcard]: found 4 wildcard dependencies for crate 'armee-dynamics'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/armee-dynamics/Cargo.toml:11:20
   │
11 │ armee-kinematics = { workspace = true }
   │                    ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   ·
20 │ marengo-config = { workspace = true }
   │                  ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:62:18
   │
62 │ armee-dynamics = { path = "crates/armee-dynamics" }
   │                  ──────────────────────────────────
   │                  │
   │                  workspace dependency
   │                  workspace dependency
   │
   ├ armee-dynamics v0.1.0
     ├── (dev) berthier v0.1.0
     │   ├── marengo-pi v0.1.0
     │   ├── motor-repl v0.1.0
     │   └── wave-demo v0.1.0
     ├── marengo-log-cli v0.1.0
     ├── marengo-pi v0.1.0 (*)
     └── motor-repl v0.1.0 (*)

warning[wildcard]: found 2 wildcard dependencies for crate 'armee-kinematics'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/armee-kinematics/Cargo.toml:18:28
   │
18 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:61:20
   │
61 │ armee-kinematics = { path = "crates/armee-kinematics" }
   │                    ──────────────────────────────────── workspace dependency
   │
   ├ armee-kinematics v0.1.0
     ├── armee-dynamics v0.1.0
     │   ├── (dev) berthier v0.1.0
     │   │   ├── marengo-pi v0.1.0
     │   │   ├── motor-repl v0.1.0
     │   │   └── wave-demo v0.1.0
     │   ├── marengo-log-cli v0.1.0
     │   ├── marengo-pi v0.1.0 (*)
     │   └── motor-repl v0.1.0 (*)
     ├── berthier v0.1.0 (*)
     ├── davout v0.1.0
     │   ├── berthier v0.1.0 (*)
     │   ├── marengo-pi v0.1.0 (*)
     │   └── motor-repl v0.1.0 (*)
     ├── marengo-config v0.1.0
     │   ├── (dev) armee-dynamics v0.1.0 (*)
     │   ├── (dev) armee-kinematics v0.1.0 (*)
     │   ├── berthier v0.1.0 (*)
     │   ├── davout v0.1.0 (*)
     │   ├── marengo-candump v0.1.0
     │   │   ├── marengo-gateway v0.1.0
     │   │   ├── marengo-log-cli v0.1.0 (*)
     │   │   └── marengo-store v0.1.0
     │   │       ├── marengo-gateway v0.1.0 (*)
     │   │       └── marengo-log-cli v0.1.0 (*)
     │   ├── marengo-gateway v0.1.0 (*)
     │   ├── marengo-homing v0.1.0
     │   │   ├── davout v0.1.0 (*)
     │   │   └── marengo-pi v0.1.0 (*)
     │   ├── marengo-limit-sync v0.1.0
     │   ├── marengo-log-cli v0.1.0 (*)
     │   ├── marengo-pi v0.1.0 (*)
     │   ├── motor-repl v0.1.0 (*)
     │   └── robstride v0.1.0
     │       ├── (dev) berthier v0.1.0 (*)
     │       ├── davout v0.1.0 (*)
     │       ├── marengo-candump v0.1.0 (*)
     │       ├── marengo-pi v0.1.0 (*)
     │       └── motor-repl v0.1.0 (*)
     ├── sim-harness v0.1.0
     └── talleyrand v0.1.0
         └── marengo-jetson v0.1.0

warning[wildcard]: found 18 wildcard dependencies for crate 'berthier'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/berthier/Cargo.toml:17:28
   │
17 │ armee-dynamics.workspace = true
   │                            ━━━━ wildcard dependency
18 │ armee-kinematics.workspace = true
   │                              ━━━━ wildcard dependency
19 │ armee-proto.workspace = true
   │                         ━━━━ wildcard dependency
20 │ chappe.workspace = true
   │                    ━━━━ wildcard dependency
21 │ davout.workspace = true
   │                    ━━━━ wildcard dependency
22 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
   ·
28 │ davout = { workspace = true, features = ["reference-journal-test-support"] }
   │          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
29 │ armee-dynamics = { workspace = true }
   │                  ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
30 │ robstride = { workspace = true }
   │             ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:64:12
   │
64 │ berthier = { path = "crates/berthier" }
   │            ────────────────────────────
   │            │
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │            workspace dependency
   │
   ├ berthier v0.1.0
     ├── marengo-pi v0.1.0
     ├── motor-repl v0.1.0
     └── wave-demo v0.1.0

warning[wildcard]: found 2 wildcard dependencies for crate 'chappe'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/chappe/Cargo.toml:14:25
   │
14 │ armee-proto.workspace = true
   │                         ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:63:10
   │
63 │ chappe = { path = "crates/chappe" }
   │          ────────────────────────── workspace dependency
   │
   ├ chappe v0.1.0
     ├── berthier v0.1.0
     │   ├── marengo-pi v0.1.0
     │   ├── motor-repl v0.1.0
     │   └── wave-demo v0.1.0
     ├── marengo-gateway v0.1.0
     ├── marengo-jetson v0.1.0
     └── marengo-pi v0.1.0 (*)

warning[wildcard]: found 8 wildcard dependencies for crate 'davout'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/davout/Cargo.toml:17:30
   │
17 │ armee-kinematics.workspace = true
   │                              ━━━━ wildcard dependency
18 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
19 │ marengo-homing.workspace = true
   │                            ━━━━ wildcard dependency
20 │ robstride.workspace = true
   │                       ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:65:10
   │
65 │ davout = { path = "crates/davout" }
   │          ──────────────────────────
   │          │
   │          workspace dependency
   │          workspace dependency
   │          workspace dependency
   │          workspace dependency
   │
   ├ davout v0.1.0
     ├── berthier v0.1.0
     │   ├── marengo-pi v0.1.0
     │   ├── motor-repl v0.1.0
     │   └── wave-demo v0.1.0
     ├── marengo-pi v0.1.0 (*)
     └── motor-repl v0.1.0 (*)

warning[duplicate]: found 3 duplicate entries for crate 'getrandom'
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:91:1
   │  
91 │ ╭ getrandom 0.2.17 registry+https://github.com/rust-lang/crates.io-index
92 │ │ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
93 │ │ getrandom 0.4.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.2.17
     └── ring v0.17.14
         ├── quinn-proto v0.11.15
         │   └── quinn v0.11.9
         │       ├── marengo-gateway v0.1.0
         │       └── web-transport-quinn v0.11.9
         │           └── marengo-gateway v0.1.0 (*)
         ├── rcgen v0.13.2
         │   └── marengo-gateway v0.1.0 (*)
         ├── rustls v0.23.45
         │   ├── axum-server v0.8.0
         │   │   └── marengo-gateway v0.1.0 (*)
         │   ├── marengo-gateway v0.1.0 (*)
         │   ├── quinn v0.11.9 (*)
         │   ├── quinn-proto v0.11.15 (*)
         │   ├── rustls-platform-verifier v0.6.2
         │   │   └── quinn-proto v0.11.15 (*)
         │   ├── tokio-rustls v0.26.4
         │   │   ├── axum-server v0.8.0 (*)
         │   │   └── (dev) marengo-gateway v0.1.0 (*)
         │   └── web-transport-quinn v0.11.9 (*)
         └── rustls-webpki v0.103.15
             ├── rustls v0.23.45 (*)
             └── rustls-platform-verifier v0.6.2 (*)
   ├ getrandom v0.3.4
     ├── fastbloom v0.14.1
     │   └── quinn-proto v0.11.15
     │       └── quinn v0.11.9
     │           ├── marengo-gateway v0.1.0
     │           └── web-transport-quinn v0.11.9
     │               └── marengo-gateway v0.1.0 (*)
     ├── jobserver v0.1.34
     │   └── cc v1.2.62
     │       ├── (build) aws-lc-sys v0.45.0
     │       │   └── aws-lc-rs v1.18.1
     │       │       ├── quinn-proto v0.11.15 (*)
     │       │       ├── rustls v0.23.45
     │       │       │   ├── axum-server v0.8.0
     │       │       │   │   └── marengo-gateway v0.1.0 (*)
     │       │       │   ├── marengo-gateway v0.1.0 (*)
     │       │       │   ├── quinn v0.11.9 (*)
     │       │       │   ├── quinn-proto v0.11.15 (*)
     │       │       │   ├── rustls-platform-verifier v0.6.2
     │       │       │   │   └── quinn-proto v0.11.15 (*)
     │       │       │   ├── tokio-rustls v0.26.4
     │       │       │   │   ├── axum-server v0.8.0 (*)
     │       │       │   │   └── (dev) marengo-gateway v0.1.0 (*)
     │       │       │   └── web-transport-quinn v0.11.9 (*)
     │       │       └── rustls-webpki v0.103.15
     │       │           ├── rustls v0.23.45 (*)
     │       │           └── rustls-platform-verifier v0.6.2 (*)
     │       ├── cmake v0.1.58
     │       │   └── (build) aws-lc-sys v0.45.0 (*)
     │       ├── (build) iana-time-zone-haiku v0.1.2
     │       │   └── iana-time-zone v0.1.65
     │       │       └── chrono v0.4.44
     │       │           └── marengo-homing v0.1.0
     │       │               ├── davout v0.1.0
     │       │               │   ├── berthier v0.1.0
     │       │               │   │   ├── marengo-pi v0.1.0
     │       │               │   │   ├── motor-repl v0.1.0
     │       │               │   │   └── wave-demo v0.1.0
     │       │               │   ├── marengo-pi v0.1.0 (*)
     │       │               │   └── motor-repl v0.1.0 (*)
     │       │               └── marengo-pi v0.1.0 (*)
     │       ├── (build) libsqlite3-sys v0.30.1
     │       │   └── rusqlite v0.32.1
     │       │       ├── davout v0.1.0 (*)
     │       │       └── marengo-store v0.1.0
     │       │           ├── marengo-gateway v0.1.0 (*)
     │       │           └── marengo-log-cli v0.1.0
     │       └── (build) ring v0.17.14
     │           ├── quinn-proto v0.11.15 (*)
     │           ├── rcgen v0.13.2
     │           │   └── marengo-gateway v0.1.0 (*)
     │           ├── rustls v0.23.45 (*)
     │           └── rustls-webpki v0.103.15 (*)
     ├── quinn-proto v0.11.15 (*)
     └── rand_core v0.9.5
         ├── rand v0.9.4
         │   ├── fastbloom v0.14.1 (*)
         │   ├── proptest v1.11.0
         │   │   └── (dev) berthier v0.1.0 (*)
         │   └── quinn-proto v0.11.15 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.4 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.2
     └── tempfile v3.27.0
         ├── (dev) chappe v0.1.0
         │   ├── berthier v0.1.0
         │   │   ├── marengo-pi v0.1.0
         │   │   ├── motor-repl v0.1.0
         │   │   └── wave-demo v0.1.0
         │   ├── marengo-gateway v0.1.0
         │   ├── marengo-jetson v0.1.0
         │   └── marengo-pi v0.1.0 (*)
         ├── (dev) marengo-config v0.1.0
         │   ├── (dev) armee-dynamics v0.1.0
         │   │   ├── (dev) berthier v0.1.0 (*)
         │   │   ├── marengo-log-cli v0.1.0
         │   │   ├── marengo-pi v0.1.0 (*)
         │   │   └── motor-repl v0.1.0 (*)
         │   ├── (dev) armee-kinematics v0.1.0
         │   │   ├── armee-dynamics v0.1.0 (*)
         │   │   ├── berthier v0.1.0 (*)
         │   │   ├── davout v0.1.0
         │   │   │   ├── berthier v0.1.0 (*)
         │   │   │   ├── marengo-pi v0.1.0 (*)
         │   │   │   └── motor-repl v0.1.0 (*)
         │   │   ├── marengo-config v0.1.0 (*)
         │   │   ├── sim-harness v0.1.0
         │   │   └── talleyrand v0.1.0
         │   │       └── marengo-jetson v0.1.0 (*)
         │   ├── berthier v0.1.0 (*)
         │   ├── davout v0.1.0 (*)
         │   ├── marengo-candump v0.1.0
         │   │   ├── marengo-gateway v0.1.0 (*)
         │   │   ├── marengo-log-cli v0.1.0 (*)
         │   │   └── marengo-store v0.1.0
         │   │       ├── marengo-gateway v0.1.0 (*)
         │   │       └── marengo-log-cli v0.1.0 (*)
         │   ├── marengo-gateway v0.1.0 (*)
         │   ├── marengo-homing v0.1.0
         │   │   ├── davout v0.1.0 (*)
         │   │   └── marengo-pi v0.1.0 (*)
         │   ├── marengo-limit-sync v0.1.0
         │   ├── marengo-log-cli v0.1.0 (*)
         │   ├── marengo-pi v0.1.0 (*)
         │   ├── motor-repl v0.1.0 (*)
         │   └── robstride v0.1.0
         │       ├── (dev) berthier v0.1.0 (*)
         │       ├── davout v0.1.0 (*)
         │       ├── marengo-candump v0.1.0 (*)
         │       ├── marengo-pi v0.1.0 (*)
         │       └── motor-repl v0.1.0 (*)
         ├── (dev) marengo-deploy v0.1.0
         │   └── marengo-gateway v0.1.0 (*)
         ├── (dev) marengo-gateway v0.1.0 (*)
         ├── (dev) marengo-log-cli v0.1.0 (*)
         ├── (dev) marengo-pi v0.1.0 (*)
         ├── marengo-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   └── (dev) berthier v0.1.0 (*)
         ├── prost-build v0.13.5
         │   └── (build) armee-proto v0.1.0
         │       ├── berthier v0.1.0 (*)
         │       ├── chappe v0.1.0 (*)
         │       ├── marengo-gateway v0.1.0 (*)
         │       ├── marengo-homing v0.1.0 (*)
         │       ├── marengo-host-metrics v0.1.0
         │       │   └── marengo-pi v0.1.0 (*)
         │       └── marengo-pi v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'hashbrown'
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:95:1
   │  
95 │ ╭ hashbrown 0.14.5 registry+https://github.com/rust-lang/crates.io-index
96 │ │ hashbrown 0.17.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ hashbrown v0.14.5
     └── hashlink v0.9.1
         └── rusqlite v0.32.1
             ├── davout v0.1.0
             │   ├── berthier v0.1.0
             │   │   ├── marengo-pi v0.1.0
             │   │   ├── motor-repl v0.1.0
             │   │   └── wave-demo v0.1.0
             │   ├── marengo-pi v0.1.0 (*)
             │   └── motor-repl v0.1.0 (*)
             └── marengo-store v0.1.0
                 ├── marengo-gateway v0.1.0
                 └── marengo-log-cli v0.1.0
   ├ hashbrown v0.17.1
     └── indexmap v2.14.0
         ├── h2 v0.4.16
         │   └── hyper v1.9.0
         │       ├── axum v0.8.9
         │       │   └── marengo-gateway v0.1.0
         │       ├── axum-server v0.8.0
         │       │   └── marengo-gateway v0.1.0 (*)
         │       └── hyper-util v0.1.20
         │           ├── axum v0.8.9 (*)
         │           └── axum-server v0.8.0 (*)
         ├── petgraph v0.7.1
         │   └── prost-build v0.13.5
         │       └── (build) armee-proto v0.1.0
         │           ├── berthier v0.1.0
         │           │   ├── marengo-pi v0.1.0
         │           │   ├── motor-repl v0.1.0
         │           │   └── wave-demo v0.1.0
         │           ├── chappe v0.1.0
         │           │   ├── berthier v0.1.0 (*)
         │           │   ├── marengo-gateway v0.1.0 (*)
         │           │   ├── marengo-jetson v0.1.0
         │           │   └── marengo-pi v0.1.0 (*)
         │           ├── marengo-gateway v0.1.0 (*)
         │           ├── marengo-homing v0.1.0
         │           │   ├── davout v0.1.0
         │           │   │   ├── berthier v0.1.0 (*)
         │           │   │   ├── marengo-pi v0.1.0 (*)
         │           │   │   └── motor-repl v0.1.0 (*)
         │           │   └── marengo-pi v0.1.0 (*)
         │           ├── marengo-host-metrics v0.1.0
         │           │   └── marengo-pi v0.1.0 (*)
         │           └── marengo-pi v0.1.0 (*)
         ├── serde_yaml v0.9.34+deprecated
         │   ├── (dev) armee-kinematics v0.1.0
         │   │   ├── armee-dynamics v0.1.0
         │   │   │   ├── (dev) berthier v0.1.0 (*)
         │   │   │   ├── marengo-log-cli v0.1.0
         │   │   │   ├── marengo-pi v0.1.0 (*)
         │   │   │   └── motor-repl v0.1.0 (*)
         │   │   ├── berthier v0.1.0 (*)
         │   │   ├── davout v0.1.0 (*)
         │   │   ├── marengo-config v0.1.0
         │   │   │   ├── (dev) armee-dynamics v0.1.0 (*)
         │   │   │   ├── (dev) armee-kinematics v0.1.0 (*)
         │   │   │   ├── berthier v0.1.0 (*)
         │   │   │   ├── davout v0.1.0 (*)
         │   │   │   ├── marengo-candump v0.1.0
         │   │   │   │   ├── marengo-gateway v0.1.0 (*)
         │   │   │   │   ├── marengo-log-cli v0.1.0 (*)
         │   │   │   │   └── marengo-store v0.1.0
         │   │   │   │       ├── marengo-gateway v0.1.0 (*)
         │   │   │   │       └── marengo-log-cli v0.1.0 (*)
         │   │   │   ├── marengo-gateway v0.1.0 (*)
         │   │   │   ├── marengo-homing v0.1.0 (*)
         │   │   │   ├── marengo-limit-sync v0.1.0
         │   │   │   ├── marengo-log-cli v0.1.0 (*)
         │   │   │   ├── marengo-pi v0.1.0 (*)
         │   │   │   ├── motor-repl v0.1.0 (*)
         │   │   │   └── robstride v0.1.0
         │   │   │       ├── (dev) berthier v0.1.0 (*)
         │   │   │       ├── davout v0.1.0 (*)
         │   │   │       ├── marengo-candump v0.1.0 (*)
         │   │   │       ├── marengo-pi v0.1.0 (*)
         │   │   │       └── motor-repl v0.1.0 (*)
         │   │   ├── sim-harness v0.1.0
         │   │   └── talleyrand v0.1.0
         │   │       └── marengo-jetson v0.1.0 (*)
         │   ├── (dev) davout v0.1.0 (*)
         │   ├── marengo-config v0.1.0 (*)
         │   ├── marengo-gateway v0.1.0 (*)
         │   └── marengo-homing v0.1.0 (*)
         └── sfv v0.14.0
             └── web-transport-proto v0.6.0
                 └── web-transport-quinn v0.11.9
                     └── marengo-gateway v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'heck'
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:98:1
   │  
98 │ ╭ heck 0.4.1 registry+https://github.com/rust-lang/crates.io-index
99 │ │ heck 0.5.0 registry+https://github.com/rust-lang/crates.io-index
   │ ╰────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ heck v0.4.1
     └── yaserde_derive v0.8.0
         └── urdf-rs v0.8.0
             ├── armee-dynamics v0.1.0
             │   ├── (dev) berthier v0.1.0
             │   │   ├── marengo-pi v0.1.0
             │   │   ├── motor-repl v0.1.0
             │   │   └── wave-demo v0.1.0
             │   ├── marengo-log-cli v0.1.0
             │   ├── marengo-pi v0.1.0 (*)
             │   └── motor-repl v0.1.0 (*)
             ├── armee-kinematics v0.1.0
             │   ├── armee-dynamics v0.1.0 (*)
             │   ├── berthier v0.1.0 (*)
             │   ├── davout v0.1.0
             │   │   ├── berthier v0.1.0 (*)
             │   │   ├── marengo-pi v0.1.0 (*)
             │   │   └── motor-repl v0.1.0 (*)
             │   ├── marengo-config v0.1.0
             │   │   ├── (dev) armee-dynamics v0.1.0 (*)
             │   │   ├── (dev) armee-kinematics v0.1.0 (*)
             │   │   ├── berthier v0.1.0 (*)
             │   │   ├── davout v0.1.0 (*)
             │   │   ├── marengo-candump v0.1.0
             │   │   │   ├── marengo-gateway v0.1.0
             │   │   │   ├── marengo-log-cli v0.1.0 (*)
             │   │   │   └── marengo-store v0.1.0
             │   │   │       ├── marengo-gateway v0.1.0 (*)
             │   │   │       └── marengo-log-cli v0.1.0 (*)
             │   │   ├── marengo-gateway v0.1.0 (*)
             │   │   ├── marengo-homing v0.1.0
             │   │   │   ├── davout v0.1.0 (*)
             │   │   │   └── marengo-pi v0.1.0 (*)
             │   │   ├── marengo-limit-sync v0.1.0
             │   │   ├── marengo-log-cli v0.1.0 (*)
             │   │   ├── marengo-pi v0.1.0 (*)
             │   │   ├── motor-repl v0.1.0 (*)
             │   │   └── robstride v0.1.0
             │   │       ├── (dev) berthier v0.1.0 (*)
             │   │       ├── davout v0.1.0 (*)
             │   │       ├── marengo-candump v0.1.0 (*)
             │   │       ├── marengo-pi v0.1.0 (*)
             │   │       └── motor-repl v0.1.0 (*)
             │   ├── sim-harness v0.1.0
             │   └── talleyrand v0.1.0
             │       └── marengo-jetson v0.1.0
             ├── davout v0.1.0 (*)
             └── marengo-config v0.1.0 (*)
   ├ heck v0.5.0
     ├── clap_derive v4.6.1
     │   └── clap v4.6.1
     │       ├── marengo-limit-sync v0.1.0
     │       └── marengo-log-cli v0.1.0
     └── prost-build v0.13.5
         └── (build) armee-proto v0.1.0
             ├── berthier v0.1.0
             │   ├── marengo-pi v0.1.0
             │   ├── motor-repl v0.1.0
             │   └── wave-demo v0.1.0
             ├── chappe v0.1.0
             │   ├── berthier v0.1.0 (*)
             │   ├── marengo-gateway v0.1.0
             │   ├── marengo-jetson v0.1.0
             │   └── marengo-pi v0.1.0 (*)
             ├── marengo-gateway v0.1.0 (*)
             ├── marengo-homing v0.1.0
             │   ├── davout v0.1.0
             │   │   ├── berthier v0.1.0 (*)
             │   │   ├── marengo-pi v0.1.0 (*)
             │   │   └── motor-repl v0.1.0 (*)
             │   └── marengo-pi v0.1.0 (*)
             ├── marengo-host-metrics v0.1.0
             │   └── marengo-pi v0.1.0 (*)
             └── marengo-pi v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/imu-probe/Cargo.toml:23:29
   │
23 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ imu-probe v0.1.0

warning[wildcard]: found 2 wildcard dependencies for crate 'imu-probe'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/imu-probe/Cargo.toml:22:15
   │
22 │ marengo-imu = { path = "../../crates/marengo-imu" }
   │               ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
23 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
   │
   ├ imu-probe v0.1.0

warning[wildcard]: found 4 wildcard dependencies for crate 'marengo-candump'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/marengo-candump/Cargo.toml:21:13
   │
21 │ robstride = { workspace = true, optional = true }
   │             ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
22 │ marengo-config = { workspace = true, optional = true }
   │                  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:72:19
   │
72 │ marengo-candump = { path = "crates/marengo-candump", default-features = false }
   │                   ─────────────────────────────────────────────────────────────
   │                   │
   │                   workspace dependency
   │                   workspace dependency
   │
   ├ marengo-candump v0.1.0
     ├── marengo-gateway v0.1.0
     ├── marengo-log-cli v0.1.0
     └── marengo-store v0.1.0
         ├── marengo-gateway v0.1.0 (*)
         └── marengo-log-cli v0.1.0 (*)

warning[wildcard]: found 2 wildcard dependencies for crate 'marengo-config'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/marengo-config/Cargo.toml:14:30
   │
14 │ armee-kinematics.workspace = true
   │                              ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:58:18
   │
58 │ marengo-config = { path = "crates/marengo-config" }
   │                  ────────────────────────────────── workspace dependency
   │
   ├ marengo-config v0.1.0
     ├── (dev) armee-dynamics v0.1.0
     │   ├── (dev) berthier v0.1.0
     │   │   ├── marengo-pi v0.1.0
     │   │   ├── motor-repl v0.1.0
     │   │   └── wave-demo v0.1.0
     │   ├── marengo-log-cli v0.1.0
     │   ├── marengo-pi v0.1.0 (*)
     │   └── motor-repl v0.1.0 (*)
     ├── (dev) armee-kinematics v0.1.0
     │   ├── armee-dynamics v0.1.0 (*)
     │   ├── berthier v0.1.0 (*)
     │   ├── davout v0.1.0
     │   │   ├── berthier v0.1.0 (*)
     │   │   ├── marengo-pi v0.1.0 (*)
     │   │   └── motor-repl v0.1.0 (*)
     │   ├── marengo-config v0.1.0 (*)
     │   ├── sim-harness v0.1.0
     │   └── talleyrand v0.1.0
     │       └── marengo-jetson v0.1.0
     ├── berthier v0.1.0 (*)
     ├── davout v0.1.0 (*)
     ├── marengo-candump v0.1.0
     │   ├── marengo-gateway v0.1.0
     │   ├── marengo-log-cli v0.1.0 (*)
     │   └── marengo-store v0.1.0
     │       ├── marengo-gateway v0.1.0 (*)
     │       └── marengo-log-cli v0.1.0 (*)
     ├── marengo-gateway v0.1.0 (*)
     ├── marengo-homing v0.1.0
     │   ├── davout v0.1.0 (*)
     │   └── marengo-pi v0.1.0 (*)
     ├── marengo-limit-sync v0.1.0
     ├── marengo-log-cli v0.1.0 (*)
     ├── marengo-pi v0.1.0 (*)
     ├── motor-repl v0.1.0 (*)
     └── robstride v0.1.0
         ├── (dev) berthier v0.1.0 (*)
         ├── davout v0.1.0 (*)
         ├── marengo-candump v0.1.0 (*)
         ├── marengo-pi v0.1.0 (*)
         └── motor-repl v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-gateway/Cargo.toml:18:25
   │
18 │ armee-proto.workspace = true
   │                         ━━━━
   │                         │
   │                         usage of workspace dependency
   │
   ├ marengo-gateway v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-gateway/Cargo.toml:25:33
   │
25 │ marengo-candump = { workspace = true, features = ["robstride-enrichment"] }
   │                   ──────────────━━━━───────────────────────────────────────
   │                                 │
   │                                 usage of workspace dependency
   │
   ├ marengo-gateway v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-gateway/Cargo.toml:24:28
   │
24 │ marengo-config.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ marengo-gateway v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-gateway/Cargo.toml:26:28
   │
26 │ marengo-deploy.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ marengo-gateway v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-gateway/Cargo.toml:27:29
   │
27 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ marengo-gateway v0.1.0 (*)

warning[wildcard]: found 7 wildcard dependencies for crate 'marengo-gateway'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-gateway/Cargo.toml:18:25
   │
18 │ armee-proto.workspace = true
   │                         ━━━━ wildcard dependency
   ·
22 │ chappe = { path = "../../crates/chappe" }
   │          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
23 │ futures = "0.3"
24 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
25 │ marengo-candump = { workspace = true, features = ["robstride-enrichment"] }
   │                   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
26 │ marengo-deploy.workspace = true
   │                            ━━━━ wildcard dependency
27 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
28 │ marengo-store = { path = "../../crates/marengo-store" }
   │                 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ├ marengo-gateway v0.1.0

warning[wildcard]: found 4 wildcard dependencies for crate 'marengo-homing'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/marengo-homing/Cargo.toml:14:25
   │
14 │ armee-proto.workspace = true
   │                         ━━━━ wildcard dependency
15 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:59:18
   │
59 │ marengo-homing = { path = "crates/marengo-homing" }
   │                  ──────────────────────────────────
   │                  │
   │                  workspace dependency
   │                  workspace dependency
   │
   ├ marengo-homing v0.1.0
     ├── davout v0.1.0
     │   ├── berthier v0.1.0
     │   │   ├── marengo-pi v0.1.0
     │   │   ├── motor-repl v0.1.0
     │   │   └── wave-demo v0.1.0
     │   ├── marengo-pi v0.1.0 (*)
     │   └── motor-repl v0.1.0 (*)
     └── marengo-pi v0.1.0 (*)

warning[wildcard]: found 2 wildcard dependencies for crate 'marengo-host-metrics'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/marengo-host-metrics/Cargo.toml:14:25
   │
14 │ armee-proto.workspace = true
   │                         ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:70:24
   │
70 │ marengo-host-metrics = { path = "crates/marengo-host-metrics" }
   │                        ──────────────────────────────────────── workspace dependency
   │
   ├ marengo-host-metrics v0.1.0
     └── marengo-pi v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-jetson/Cargo.toml:20:29
   │
20 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ marengo-jetson v0.1.0

warning[wildcard]: found 4 wildcard dependencies for crate 'marengo-jetson'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-jetson/Cargo.toml:18:10
   │
18 │ chappe = { path = "../../crates/chappe" }
   │          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
19 │ fouche = { path = "../../crates/fouche" }
   │          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
20 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
21 │ talleyrand = { path = "../../crates/talleyrand" }
   │              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ├ marengo-jetson v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-limit-sync/Cargo.toml:20:28
   │
20 │ marengo-config.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ marengo-limit-sync v0.1.0

warning[wildcard]: found 1 wildcard dependency for crate 'marengo-limit-sync'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-limit-sync/Cargo.toml:20:28
   │
20 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
   │
   ├ marengo-limit-sync v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-log-cli/Cargo.toml:22:32
   │
22 │ armee-dynamics = { workspace = true }
   │                  ──────────────━━━━──
   │                                │
   │                                usage of workspace dependency
   │
   ├ marengo-log-cli v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-log-cli/Cargo.toml:24:33
   │
24 │ marengo-candump = { workspace = true }
   │                   ──────────────━━━━──
   │                                 │
   │                                 usage of workspace dependency
   │
   ├ marengo-log-cli v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-log-cli/Cargo.toml:25:32
   │
25 │ marengo-config = { workspace = true }
   │                  ──────────────━━━━──
   │                                │
   │                                usage of workspace dependency
   │
   ├ marengo-log-cli v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-log-cli/Cargo.toml:31:29
   │
31 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ marengo-log-cli v0.1.0 (*)

warning[wildcard]: found 5 wildcard dependencies for crate 'marengo-log-cli'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-log-cli/Cargo.toml:22:18
   │
22 │ armee-dynamics = { workspace = true }
   │                  ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
23 │ clap = { version = "4", features = ["derive", "env"] }
24 │ marengo-candump = { workspace = true }
   │                   ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
25 │ marengo-config = { workspace = true }
   │                  ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
26 │ marengo-store = { path = "../../crates/marengo-store" }
   │                 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   ·
31 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
   │
   ├ marengo-log-cli v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:23:28
   │
23 │ armee-dynamics.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ marengo-pi v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:24:25
   │
24 │ armee-proto.workspace = true
   │                         ━━━━
   │                         │
   │                         usage of workspace dependency
   │
   ├ marengo-pi v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:29:28
   │
29 │ marengo-config.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ marengo-pi v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:30:28
   │
30 │ marengo-homing.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ marengo-pi v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:33:34
   │
33 │ marengo-host-metrics.workspace = true
   │                                  ━━━━
   │                                  │
   │                                  usage of workspace dependency
   │
   ├ marengo-pi v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:32:29
   │
32 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ marengo-pi v0.1.0 (*)

warning[wildcard]: found 11 wildcard dependencies for crate 'marengo-pi'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/marengo-pi/Cargo.toml:23:28
   │
23 │ armee-dynamics.workspace = true
   │                            ━━━━ wildcard dependency
24 │ armee-proto.workspace = true
   │                         ━━━━ wildcard dependency
25 │ berthier = { path = "../../crates/berthier" }
   │            ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
26 │ chappe = { path = "../../crates/chappe" }
   │          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
27 │ ctrlc = "3.4"
28 │ davout = { path = "../../crates/davout" }
   │          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
29 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
30 │ marengo-homing.workspace = true
   │                            ━━━━ wildcard dependency
31 │ marengo-imu = { path = "../../crates/marengo-imu", optional = true }
32 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
33 │ marengo-host-metrics.workspace = true
   │                                  ━━━━ wildcard dependency
34 │ robstride = { path = "../../crates/robstride" }
   │             ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   ·
41 │ berthier = { path = "../../crates/berthier", features = ["reference-journal-test-support"] }
   │            ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ├ marengo-pi v0.1.0

warning[wildcard]: found 2 wildcard dependencies for crate 'marengo-store'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/marengo-store/Cargo.toml:15:29
   │
15 │ marengo-candump.workspace = true
   │                             ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:71:17
   │
71 │ marengo-store = { path = "crates/marengo-store" }
   │                 ───────────────────────────────── workspace dependency
   │
   ├ marengo-store v0.1.0
     ├── marengo-gateway v0.1.0
     └── marengo-log-cli v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/motor-repl/Cargo.toml:23:28
   │
23 │ armee-dynamics.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ motor-repl v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/motor-repl/Cargo.toml:25:20
   │
25 │ davout.workspace = true
   │                    ━━━━
   │                    │
   │                    usage of workspace dependency
   │
   ├ motor-repl v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/motor-repl/Cargo.toml:26:28
   │
26 │ marengo-config.workspace = true
   │                            ━━━━
   │                            │
   │                            usage of workspace dependency
   │
   ├ motor-repl v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/motor-repl/Cargo.toml:27:29
   │
27 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ motor-repl v0.1.0 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/motor-repl/Cargo.toml:28:23
   │
28 │ robstride.workspace = true
   │                       ━━━━
   │                       │
   │                       usage of workspace dependency
   │
   ├ motor-repl v0.1.0 (*)

warning[wildcard]: found 6 wildcard dependencies for crate 'motor-repl'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/motor-repl/Cargo.toml:23:28
   │
23 │ armee-dynamics.workspace = true
   │                            ━━━━ wildcard dependency
24 │ berthier = { path = "../../crates/berthier" }
   │            ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
25 │ davout.workspace = true
   │                    ━━━━ wildcard dependency
26 │ marengo-config.workspace = true
   │                            ━━━━ wildcard dependency
27 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
28 │ robstride.workspace = true
   │                       ━━━━ wildcard dependency
   │
   ├ motor-repl v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/probe/Cargo.toml:18:29
   │
18 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ probe v0.1.0

warning[wildcard]: found 1 wildcard dependency for crate 'probe'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/probe/Cargo.toml:18:29
   │
18 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
   │
   ├ probe v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'r-efi'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:207:1
    │  
207 │ ╭ r-efi 5.3.0 registry+https://github.com/rust-lang/crates.io-index
208 │ │ r-efi 6.0.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰─────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ r-efi v5.3.0
      └── getrandom v0.3.4
          ├── fastbloom v0.14.1
          │   └── quinn-proto v0.11.15
          │       └── quinn v0.11.9
          │           ├── marengo-gateway v0.1.0
          │           └── web-transport-quinn v0.11.9
          │               └── marengo-gateway v0.1.0 (*)
          ├── jobserver v0.1.34
          │   └── cc v1.2.62
          │       ├── (build) aws-lc-sys v0.45.0
          │       │   └── aws-lc-rs v1.18.1
          │       │       ├── quinn-proto v0.11.15 (*)
          │       │       ├── rustls v0.23.45
          │       │       │   ├── axum-server v0.8.0
          │       │       │   │   └── marengo-gateway v0.1.0 (*)
          │       │       │   ├── marengo-gateway v0.1.0 (*)
          │       │       │   ├── quinn v0.11.9 (*)
          │       │       │   ├── quinn-proto v0.11.15 (*)
          │       │       │   ├── rustls-platform-verifier v0.6.2
          │       │       │   │   └── quinn-proto v0.11.15 (*)
          │       │       │   ├── tokio-rustls v0.26.4
          │       │       │   │   ├── axum-server v0.8.0 (*)
          │       │       │   │   └── (dev) marengo-gateway v0.1.0 (*)
          │       │       │   └── web-transport-quinn v0.11.9 (*)
          │       │       └── rustls-webpki v0.103.15
          │       │           ├── rustls v0.23.45 (*)
          │       │           └── rustls-platform-verifier v0.6.2 (*)
          │       ├── cmake v0.1.58
          │       │   └── (build) aws-lc-sys v0.45.0 (*)
          │       ├── (build) iana-time-zone-haiku v0.1.2
          │       │   └── iana-time-zone v0.1.65
          │       │       └── chrono v0.4.44
          │       │           └── marengo-homing v0.1.0
          │       │               ├── davout v0.1.0
          │       │               │   ├── berthier v0.1.0
          │       │               │   │   ├── marengo-pi v0.1.0
          │       │               │   │   ├── motor-repl v0.1.0
          │       │               │   │   └── wave-demo v0.1.0
          │       │               │   ├── marengo-pi v0.1.0 (*)
          │       │               │   └── motor-repl v0.1.0 (*)
          │       │               └── marengo-pi v0.1.0 (*)
          │       ├── (build) libsqlite3-sys v0.30.1
          │       │   └── rusqlite v0.32.1
          │       │       ├── davout v0.1.0 (*)
          │       │       └── marengo-store v0.1.0
          │       │           ├── marengo-gateway v0.1.0 (*)
          │       │           └── marengo-log-cli v0.1.0
          │       └── (build) ring v0.17.14
          │           ├── quinn-proto v0.11.15 (*)
          │           ├── rcgen v0.13.2
          │           │   └── marengo-gateway v0.1.0 (*)
          │           ├── rustls v0.23.45 (*)
          │           └── rustls-webpki v0.103.15 (*)
          ├── quinn-proto v0.11.15 (*)
          └── rand_core v0.9.5
              ├── rand v0.9.4
              │   ├── fastbloom v0.14.1 (*)
              │   ├── proptest v1.11.0
              │   │   └── (dev) berthier v0.1.0 (*)
              │   └── quinn-proto v0.11.15 (*)
              ├── rand_chacha v0.9.0
              │   ├── proptest v1.11.0 (*)
              │   └── rand v0.9.4 (*)
              └── rand_xorshift v0.4.0
                  └── proptest v1.11.0 (*)
    ├ r-efi v6.0.0
      └── getrandom v0.4.2
          └── tempfile v3.27.0
              ├── (dev) chappe v0.1.0
              │   ├── berthier v0.1.0
              │   │   ├── marengo-pi v0.1.0
              │   │   ├── motor-repl v0.1.0
              │   │   └── wave-demo v0.1.0
              │   ├── marengo-gateway v0.1.0
              │   ├── marengo-jetson v0.1.0
              │   └── marengo-pi v0.1.0 (*)
              ├── (dev) marengo-config v0.1.0
              │   ├── (dev) armee-dynamics v0.1.0
              │   │   ├── (dev) berthier v0.1.0 (*)
              │   │   ├── marengo-log-cli v0.1.0
              │   │   ├── marengo-pi v0.1.0 (*)
              │   │   └── motor-repl v0.1.0 (*)
              │   ├── (dev) armee-kinematics v0.1.0
              │   │   ├── armee-dynamics v0.1.0 (*)
              │   │   ├── berthier v0.1.0 (*)
              │   │   ├── davout v0.1.0
              │   │   │   ├── berthier v0.1.0 (*)
              │   │   │   ├── marengo-pi v0.1.0 (*)
              │   │   │   └── motor-repl v0.1.0 (*)
              │   │   ├── marengo-config v0.1.0 (*)
              │   │   ├── sim-harness v0.1.0
              │   │   └── talleyrand v0.1.0
              │   │       └── marengo-jetson v0.1.0 (*)
              │   ├── berthier v0.1.0 (*)
              │   ├── davout v0.1.0 (*)
              │   ├── marengo-candump v0.1.0
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-log-cli v0.1.0 (*)
              │   │   └── marengo-store v0.1.0
              │   │       ├── marengo-gateway v0.1.0 (*)
              │   │       └── marengo-log-cli v0.1.0 (*)
              │   ├── marengo-gateway v0.1.0 (*)
              │   ├── marengo-homing v0.1.0
              │   │   ├── davout v0.1.0 (*)
              │   │   └── marengo-pi v0.1.0 (*)
              │   ├── marengo-limit-sync v0.1.0
              │   ├── marengo-log-cli v0.1.0 (*)
              │   ├── marengo-pi v0.1.0 (*)
              │   ├── motor-repl v0.1.0 (*)
              │   └── robstride v0.1.0
              │       ├── (dev) berthier v0.1.0 (*)
              │       ├── davout v0.1.0 (*)
              │       ├── marengo-candump v0.1.0 (*)
              │       ├── marengo-pi v0.1.0 (*)
              │       └── motor-repl v0.1.0 (*)
              ├── (dev) marengo-deploy v0.1.0
              │   └── marengo-gateway v0.1.0 (*)
              ├── (dev) marengo-gateway v0.1.0 (*)
              ├── (dev) marengo-log-cli v0.1.0 (*)
              ├── (dev) marengo-pi v0.1.0 (*)
              ├── marengo-store v0.1.0 (*)
              ├── proptest v1.11.0
              │   └── (dev) berthier v0.1.0 (*)
              ├── prost-build v0.13.5
              │   └── (build) armee-proto v0.1.0
              │       ├── berthier v0.1.0 (*)
              │       ├── chappe v0.1.0 (*)
              │       ├── marengo-gateway v0.1.0 (*)
              │       ├── marengo-homing v0.1.0 (*)
              │       ├── marengo-host-metrics v0.1.0
              │       │   └── marengo-pi v0.1.0 (*)
              │       └── marengo-pi v0.1.0 (*)
              └── rusty-fork v0.3.1
                  └── proptest v1.11.0 (*)

warning[wildcard]: found 2 wildcard dependencies for crate 'robstride'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/robstride/Cargo.toml:19:18
   │
19 │ marengo-config = { workspace = true }
   │                  ━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:68:13
   │
68 │ robstride = { path = "crates/robstride" }
   │             ───────────────────────────── workspace dependency
   │
   ├ robstride v0.1.0
     ├── (dev) berthier v0.1.0
     │   ├── marengo-pi v0.1.0
     │   ├── motor-repl v0.1.0
     │   └── wave-demo v0.1.0
     ├── davout v0.1.0
     │   ├── berthier v0.1.0 (*)
     │   ├── marengo-pi v0.1.0 (*)
     │   └── motor-repl v0.1.0 (*)
     ├── marengo-candump v0.1.0
     │   ├── marengo-gateway v0.1.0
     │   ├── marengo-log-cli v0.1.0
     │   └── marengo-store v0.1.0
     │       ├── marengo-gateway v0.1.0 (*)
     │       └── marengo-log-cli v0.1.0 (*)
     ├── marengo-pi v0.1.0 (*)
     └── motor-repl v0.1.0 (*)

warning[wildcard]: found 1 wildcard dependency for crate 'sim-harness'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/sim-harness/Cargo.toml:14:20
   │
14 │ armee-kinematics = { path = "../armee-kinematics" }
   │                    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
   │
   ├ sim-harness v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'socket2'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:261:1
    │  
261 │ ╭ socket2 0.5.10 registry+https://github.com/rust-lang/crates.io-index
262 │ │ socket2 0.6.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ socket2 v0.5.10
      ├── quinn v0.11.9
      │   ├── marengo-gateway v0.1.0
      │   └── web-transport-quinn v0.11.9
      │       └── marengo-gateway v0.1.0 (*)
      └── quinn-udp v0.5.14
          └── quinn v0.11.9 (*)
    ├ socket2 v0.6.3
      └── tokio v1.52.3
          ├── axum v0.8.9
          │   └── marengo-gateway v0.1.0
          ├── axum-server v0.8.0
          │   └── marengo-gateway v0.1.0 (*)
          ├── chappe v0.1.0
          │   ├── berthier v0.1.0
          │   │   ├── marengo-pi v0.1.0
          │   │   ├── motor-repl v0.1.0
          │   │   └── wave-demo v0.1.0
          │   ├── marengo-gateway v0.1.0 (*)
          │   ├── marengo-jetson v0.1.0
          │   └── marengo-pi v0.1.0 (*)
          ├── fs-err v3.3.0
          │   └── axum-server v0.8.0 (*)
          ├── h2 v0.4.16
          │   └── hyper v1.9.0
          │       ├── axum v0.8.9 (*)
          │       ├── axum-server v0.8.0 (*)
          │       └── hyper-util v0.1.20
          │           ├── axum v0.8.9 (*)
          │           └── axum-server v0.8.0 (*)
          ├── hyper v1.9.0 (*)
          ├── hyper-util v0.1.20 (*)
          ├── marengo-deploy v0.1.0
          │   └── marengo-gateway v0.1.0 (*)
          ├── marengo-gateway v0.1.0 (*)
          ├── marengo-jetson v0.1.0 (*)
          ├── marengo-pi v0.1.0 (*)
          ├── quinn v0.11.9
          │   ├── marengo-gateway v0.1.0 (*)
          │   └── web-transport-quinn v0.11.9
          │       └── marengo-gateway v0.1.0 (*)
          ├── tokio-rustls v0.26.4
          │   ├── axum-server v0.8.0 (*)
          │   └── (dev) marengo-gateway v0.1.0 (*)
          ├── tokio-util v0.7.18
          │   ├── h2 v0.4.16 (*)
          │   ├── marengo-gateway v0.1.0 (*)
          │   └── tower-http v0.6.11
          │       └── marengo-gateway v0.1.0 (*)
          ├── tower v0.5.3
          │   ├── axum v0.8.9 (*)
          │   └── (dev) marengo-gateway v0.1.0 (*)
          ├── tower-http v0.6.11 (*)
          ├── web-transport-proto v0.6.0
          │   └── web-transport-quinn v0.11.9 (*)
          └── web-transport-quinn v0.11.9 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:266:1
    │  
266 │ ╭ syn 1.0.109 registry+https://github.com/rust-lang/crates.io-index
267 │ │ syn 2.0.117 registry+https://github.com/rust-lang/crates.io-index
    │ ╰─────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v1.0.109
      └── yaserde_derive v0.8.0
          └── urdf-rs v0.8.0
              ├── armee-dynamics v0.1.0
              │   ├── (dev) berthier v0.1.0
              │   │   ├── marengo-pi v0.1.0
              │   │   ├── motor-repl v0.1.0
              │   │   └── wave-demo v0.1.0
              │   ├── marengo-log-cli v0.1.0
              │   ├── marengo-pi v0.1.0 (*)
              │   └── motor-repl v0.1.0 (*)
              ├── armee-kinematics v0.1.0
              │   ├── armee-dynamics v0.1.0 (*)
              │   ├── berthier v0.1.0 (*)
              │   ├── davout v0.1.0
              │   │   ├── berthier v0.1.0 (*)
              │   │   ├── marengo-pi v0.1.0 (*)
              │   │   └── motor-repl v0.1.0 (*)
              │   ├── marengo-config v0.1.0
              │   │   ├── (dev) armee-dynamics v0.1.0 (*)
              │   │   ├── (dev) armee-kinematics v0.1.0 (*)
              │   │   ├── berthier v0.1.0 (*)
              │   │   ├── davout v0.1.0 (*)
              │   │   ├── marengo-candump v0.1.0
              │   │   │   ├── marengo-gateway v0.1.0
              │   │   │   ├── marengo-log-cli v0.1.0 (*)
              │   │   │   └── marengo-store v0.1.0
              │   │   │       ├── marengo-gateway v0.1.0 (*)
              │   │   │       └── marengo-log-cli v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-homing v0.1.0
              │   │   │   ├── davout v0.1.0 (*)
              │   │   │   └── marengo-pi v0.1.0 (*)
              │   │   ├── marengo-limit-sync v0.1.0
              │   │   ├── marengo-log-cli v0.1.0 (*)
              │   │   ├── marengo-pi v0.1.0 (*)
              │   │   ├── motor-repl v0.1.0 (*)
              │   │   └── robstride v0.1.0
              │   │       ├── (dev) berthier v0.1.0 (*)
              │   │       ├── davout v0.1.0 (*)
              │   │       ├── marengo-candump v0.1.0 (*)
              │   │       ├── marengo-pi v0.1.0 (*)
              │   │       └── motor-repl v0.1.0 (*)
              │   ├── sim-harness v0.1.0
              │   └── talleyrand v0.1.0
              │       └── marengo-jetson v0.1.0
              ├── davout v0.1.0 (*)
              └── marengo-config v0.1.0 (*)
    ├ syn v2.0.117
      ├── asn1-rs-derive v0.5.1
      │   └── asn1-rs v0.6.2
      │       ├── der-parser v9.0.0
      │       │   └── x509-parser v0.16.0
      │       │       └── marengo-gateway v0.1.0
      │       ├── oid-registry v0.7.1
      │       │   └── x509-parser v0.16.0 (*)
      │       └── x509-parser v0.16.0 (*)
      ├── asn1-rs-impl v0.2.0
      │   └── asn1-rs v0.6.2 (*)
      ├── clap_derive v4.6.1
      │   └── clap v4.6.1
      │       ├── marengo-limit-sync v0.1.0
      │       └── marengo-log-cli v0.1.0
      ├── displaydoc v0.2.5
      │   ├── asn1-rs v0.6.2 (*)
      │   ├── der-parser v9.0.0 (*)
      │   ├── icu_collections v2.2.0
      │   │   ├── icu_normalizer v2.2.0
      │   │   │   └── idna_adapter v1.2.2
      │   │   │       └── idna v1.1.0
      │   │   │           └── url v2.5.8
      │   │   │               ├── (dev) marengo-gateway v0.1.0 (*)
      │   │   │               ├── web-transport-proto v0.6.0
      │   │   │               │   └── web-transport-quinn v0.11.9
      │   │   │               │       └── marengo-gateway v0.1.0 (*)
      │   │   │               └── web-transport-quinn v0.11.9 (*)
      │   │   └── icu_properties v2.2.0
      │   │       └── idna_adapter v1.2.2 (*)
      │   ├── icu_locale_core v2.2.0
      │   │   ├── icu_properties v2.2.0 (*)
      │   │   └── icu_provider v2.2.0
      │   │       ├── icu_normalizer v2.2.0 (*)
      │   │       └── icu_properties v2.2.0 (*)
      │   ├── icu_provider v2.2.0 (*)
      │   ├── tinystr v0.8.3
      │   │   └── icu_locale_core v2.2.0 (*)
      │   └── zerotrie v0.2.4
      │       ├── icu_properties v2.2.0 (*)
      │       └── icu_provider v2.2.0 (*)
      ├── futures-macro v0.3.32
      │   └── futures-util v0.3.32
      │       ├── axum v0.8.9
      │       │   └── marengo-gateway v0.1.0 (*)
      │       ├── futures v0.3.32
      │       │   ├── marengo-gateway v0.1.0 (*)
      │       │   └── web-transport-quinn v0.11.9 (*)
      │       ├── futures-executor v0.3.32
      │       │   └── futures v0.3.32 (*)
      │       ├── js-sys v0.3.99
      │       │   ├── getrandom v0.2.17
      │       │   │   └── ring v0.17.14
      │       │   │       ├── quinn-proto v0.11.15
      │       │   │       │   └── quinn v0.11.9
      │       │   │       │       ├── marengo-gateway v0.1.0 (*)
      │       │   │       │       └── web-transport-quinn v0.11.9 (*)
      │       │   │       ├── rcgen v0.13.2
      │       │   │       │   └── marengo-gateway v0.1.0 (*)
      │       │   │       ├── rustls v0.23.45
      │       │   │       │   ├── axum-server v0.8.0
      │       │   │       │   │   └── marengo-gateway v0.1.0 (*)
      │       │   │       │   ├── marengo-gateway v0.1.0 (*)
      │       │   │       │   ├── quinn v0.11.9 (*)
      │       │   │       │   ├── quinn-proto v0.11.15 (*)
      │       │   │       │   ├── rustls-platform-verifier v0.6.2
      │       │   │       │   │   └── quinn-proto v0.11.15 (*)
      │       │   │       │   ├── tokio-rustls v0.26.4
      │       │   │       │   │   ├── axum-server v0.8.0 (*)
      │       │   │       │   │   └── (dev) marengo-gateway v0.1.0 (*)
      │       │   │       │   └── web-transport-quinn v0.11.9 (*)
      │       │   │       └── rustls-webpki v0.103.15
      │       │   │           ├── rustls v0.23.45 (*)
      │       │   │           └── rustls-platform-verifier v0.6.2 (*)
      │       │   ├── getrandom v0.3.4
      │       │   │   ├── fastbloom v0.14.1
      │       │   │   │   └── quinn-proto v0.11.15 (*)
      │       │   │   ├── jobserver v0.1.34
      │       │   │   │   └── cc v1.2.62
      │       │   │   │       ├── (build) aws-lc-sys v0.45.0
      │       │   │   │       │   └── aws-lc-rs v1.18.1
      │       │   │   │       │       ├── quinn-proto v0.11.15 (*)
      │       │   │   │       │       ├── rustls v0.23.45 (*)
      │       │   │   │       │       └── rustls-webpki v0.103.15 (*)
      │       │   │   │       ├── cmake v0.1.58
      │       │   │   │       │   └── (build) aws-lc-sys v0.45.0 (*)
      │       │   │   │       ├── (build) iana-time-zone-haiku v0.1.2
      │       │   │   │       │   └── iana-time-zone v0.1.65
      │       │   │   │       │       └── chrono v0.4.44
      │       │   │   │       │           └── marengo-homing v0.1.0
      │       │   │   │       │               ├── davout v0.1.0
      │       │   │   │       │               │   ├── berthier v0.1.0
      │       │   │   │       │               │   │   ├── marengo-pi v0.1.0
      │       │   │   │       │               │   │   ├── motor-repl v0.1.0
      │       │   │   │       │               │   │   └── wave-demo v0.1.0
      │       │   │   │       │               │   ├── marengo-pi v0.1.0 (*)
      │       │   │   │       │               │   └── motor-repl v0.1.0 (*)
      │       │   │   │       │               └── marengo-pi v0.1.0 (*)
      │       │   │   │       ├── (build) libsqlite3-sys v0.30.1
      │       │   │   │       │   └── rusqlite v0.32.1
      │       │   │   │       │       ├── davout v0.1.0 (*)
      │       │   │   │       │       └── marengo-store v0.1.0
      │       │   │   │       │           ├── marengo-gateway v0.1.0 (*)
      │       │   │   │       │           └── marengo-log-cli v0.1.0 (*)
      │       │   │   │       └── (build) ring v0.17.14 (*)
      │       │   │   ├── quinn-proto v0.11.15 (*)
      │       │   │   └── rand_core v0.9.5
      │       │   │       ├── rand v0.9.4
      │       │   │       │   ├── fastbloom v0.14.1 (*)
      │       │   │       │   ├── proptest v1.11.0
      │       │   │       │   │   └── (dev) berthier v0.1.0 (*)
      │       │   │       │   └── quinn-proto v0.11.15 (*)
      │       │   │       ├── rand_chacha v0.9.0
      │       │   │       │   ├── proptest v1.11.0 (*)
      │       │   │       │   └── rand v0.9.4 (*)
      │       │   │       └── rand_xorshift v0.4.0
      │       │   │           └── proptest v1.11.0 (*)
      │       │   ├── iana-time-zone v0.1.65 (*)
      │       │   └── web-time v1.1.0
      │       │       ├── quinn v0.11.9 (*)
      │       │       ├── quinn-proto v0.11.15 (*)
      │       │       └── rustls-pki-types v1.14.1
      │       │           ├── axum-server v0.8.0 (*)
      │       │           ├── quinn-proto v0.11.15 (*)
      │       │           ├── rcgen v0.13.2 (*)
      │       │           ├── rustls v0.23.45 (*)
      │       │           ├── rustls-native-certs v0.8.3
      │       │           │   ├── rustls-platform-verifier v0.6.2 (*)
      │       │           │   └── web-transport-quinn v0.11.9 (*)
      │       │           ├── rustls-webpki v0.103.15 (*)
      │       │           └── webpki-root-certs v1.0.7
      │       │               └── rustls-platform-verifier v0.6.2 (*)
      │       ├── tower v0.5.3
      │       │   ├── axum v0.8.9 (*)
      │       │   └── (dev) marengo-gateway v0.1.0 (*)
      │       └── tower-http v0.6.11
      │           └── marengo-gateway v0.1.0 (*)
      ├── jni-sys-macros v0.4.1
      │   └── jni-sys v0.4.1
      │       └── jni-sys v0.3.1
      │           └── jni v0.21.1
      │               └── rustls-platform-verifier v0.6.2 (*)
      ├── nalgebra-macros v0.2.2
      │   └── nalgebra v0.33.3
      │       └── armee-dynamics v0.1.0
      │           ├── (dev) berthier v0.1.0 (*)
      │           ├── marengo-log-cli v0.1.0 (*)
      │           ├── marengo-pi v0.1.0 (*)
      │           └── motor-repl v0.1.0 (*)
      ├── prettyplease v0.2.37
      │   └── prost-build v0.13.5
      │       └── (build) armee-proto v0.1.0
      │           ├── berthier v0.1.0 (*)
      │           ├── chappe v0.1.0
      │           │   ├── berthier v0.1.0 (*)
      │           │   ├── marengo-gateway v0.1.0 (*)
      │           │   ├── marengo-jetson v0.1.0
      │           │   └── marengo-pi v0.1.0 (*)
      │           ├── marengo-gateway v0.1.0 (*)
      │           ├── marengo-homing v0.1.0 (*)
      │           ├── marengo-host-metrics v0.1.0
      │           │   └── marengo-pi v0.1.0 (*)
      │           └── marengo-pi v0.1.0 (*)
      ├── prost-build v0.13.5 (*)
      ├── prost-derive v0.13.5
      │   └── prost v0.13.5
      │       ├── armee-proto v0.1.0 (*)
      │       ├── marengo-gateway v0.1.0 (*)
      │       ├── prost-build v0.13.5 (*)
      │       └── prost-types v0.13.5
      │           └── prost-build v0.13.5 (*)
      ├── ref-cast-impl v1.0.25
      │   └── ref-cast v1.0.25
      │       └── sfv v0.14.0
      │           └── web-transport-proto v0.6.0 (*)
      ├── serde_derive v1.0.228
      │   └── serde v1.0.228
      │       ├── davout v0.1.0 (*)
      │       ├── marengo-candump v0.1.0
      │       │   ├── marengo-gateway v0.1.0 (*)
      │       │   ├── marengo-log-cli v0.1.0 (*)
      │       │   └── marengo-store v0.1.0 (*)
      │       ├── marengo-config v0.1.0
      │       │   ├── (dev) armee-dynamics v0.1.0 (*)
      │       │   ├── (dev) armee-kinematics v0.1.0
      │       │   │   ├── armee-dynamics v0.1.0 (*)
      │       │   │   ├── berthier v0.1.0 (*)
      │       │   │   ├── davout v0.1.0 (*)
      │       │   │   ├── marengo-config v0.1.0 (*)
      │       │   │   ├── sim-harness v0.1.0
      │       │   │   └── talleyrand v0.1.0
      │       │   │       └── marengo-jetson v0.1.0 (*)
      │       │   ├── berthier v0.1.0 (*)
      │       │   ├── davout v0.1.0 (*)
      │       │   ├── marengo-candump v0.1.0 (*)
      │       │   ├── marengo-gateway v0.1.0 (*)
      │       │   ├── marengo-homing v0.1.0 (*)
      │       │   ├── marengo-limit-sync v0.1.0 (*)
      │       │   ├── marengo-log-cli v0.1.0 (*)
      │       │   ├── marengo-pi v0.1.0 (*)
      │       │   ├── motor-repl v0.1.0 (*)
      │       │   └── robstride v0.1.0
      │       │       ├── (dev) berthier v0.1.0 (*)
      │       │       ├── davout v0.1.0 (*)
      │       │       ├── marengo-candump v0.1.0 (*)
      │       │       ├── marengo-pi v0.1.0 (*)
      │       │       └── motor-repl v0.1.0 (*)
      │       ├── marengo-deploy v0.1.0
      │       │   └── marengo-gateway v0.1.0 (*)
      │       ├── marengo-gateway v0.1.0 (*)
      │       ├── marengo-homing v0.1.0 (*)
      │       ├── marengo-log-cli v0.1.0 (*)
      │       ├── marengo-store v0.1.0 (*)
      │       ├── serde_urlencoded v0.7.1
      │       │   └── axum v0.8.9 (*)
      │       ├── serde_yaml v0.9.34+deprecated
      │       │   ├── (dev) armee-kinematics v0.1.0 (*)
      │       │   ├── (dev) davout v0.1.0 (*)
      │       │   ├── marengo-config v0.1.0 (*)
      │       │   ├── marengo-gateway v0.1.0 (*)
      │       │   └── marengo-homing v0.1.0 (*)
      │       ├── tracing-serde v0.2.0
      │       │   └── tracing-subscriber v0.3.23
      │       │       ├── chappe v0.1.0 (*)
      │       │       └── marengo-support v0.1.0
      │       │           ├── imu-probe v0.1.0
      │       │           ├── marengo-gateway v0.1.0 (*)
      │       │           ├── marengo-jetson v0.1.0 (*)
      │       │           ├── marengo-log-cli v0.1.0 (*)
      │       │           ├── marengo-pi v0.1.0 (*)
      │       │           ├── motor-repl v0.1.0 (*)
      │       │           ├── probe v0.1.0
      │       │           ├── teleop v0.1.0
      │       │           └── wave-demo v0.1.0 (*)
      │       └── tracing-subscriber v0.3.23 (*)
      ├── synstructure v0.13.2
      │   ├── asn1-rs-derive v0.5.1 (*)
      │   ├── yoke-derive v0.8.2
      │   │   └── yoke v0.8.3
      │   │       ├── icu_collections v2.2.0 (*)
      │   │       ├── icu_provider v2.2.0 (*)
      │   │       ├── zerotrie v0.2.4 (*)
      │   │       └── zerovec v0.11.6
      │   │           ├── icu_collections v2.2.0 (*)
      │   │           ├── icu_locale_core v2.2.0 (*)
      │   │           ├── icu_normalizer v2.2.0 (*)
      │   │           ├── icu_properties v2.2.0 (*)
      │   │           ├── icu_provider v2.2.0 (*)
      │   │           ├── potential_utf v0.1.5
      │   │           │   └── icu_collections v2.2.0 (*)
      │   │           └── tinystr v0.8.3 (*)
      │   └── zerofrom-derive v0.1.7
      │       └── zerofrom v0.1.8
      │           ├── icu_collections v2.2.0 (*)
      │           ├── icu_provider v2.2.0 (*)
      │           ├── yoke v0.8.3 (*)
      │           ├── zerotrie v0.2.4 (*)
      │           └── zerovec v0.11.6 (*)
      ├── thiserror-impl v1.0.69
      │   └── thiserror v1.0.69
      │       ├── armee-dynamics v0.1.0 (*)
      │       ├── armee-kinematics v0.1.0 (*)
      │       ├── asn1-rs v0.6.2 (*)
      │       ├── berthier v0.1.0 (*)
      │       ├── chappe v0.1.0 (*)
      │       ├── davout v0.1.0 (*)
      │       ├── fouche v0.1.0
      │       │   └── marengo-jetson v0.1.0 (*)
      │       ├── jni v0.21.1 (*)
      │       ├── marengo-candump v0.1.0 (*)
      │       ├── marengo-config v0.1.0 (*)
      │       ├── marengo-deploy v0.1.0 (*)
      │       ├── marengo-gateway v0.1.0 (*)
      │       ├── marengo-homing v0.1.0 (*)
      │       ├── marengo-host-metrics v0.1.0 (*)
      │       ├── marengo-imu v0.1.0
      │       │   └── imu-probe v0.1.0 (*)
      │       ├── marengo-log-cli v0.1.0 (*)
      │       ├── marengo-pi v0.1.0 (*)
      │       ├── marengo-store v0.1.0 (*)
      │       ├── robstride v0.1.0 (*)
      │       ├── talleyrand v0.1.0 (*)
      │       ├── urdf-rs v0.8.0
      │       │   ├── armee-dynamics v0.1.0 (*)
      │       │   ├── armee-kinematics v0.1.0 (*)
      │       │   ├── davout v0.1.0 (*)
      │       │   └── marengo-config v0.1.0 (*)
      │       └── x509-parser v0.16.0 (*)
      ├── thiserror-impl v2.0.18
      │   └── thiserror v2.0.18
      │       ├── quinn v0.11.9 (*)
      │       ├── quinn-proto v0.11.15 (*)
      │       ├── web-transport-proto v0.6.0 (*)
      │       └── web-transport-quinn v0.11.9 (*)
      ├── tokio-macros v2.7.0
      │   └── tokio v1.52.3
      │       ├── axum v0.8.9 (*)
      │       ├── axum-server v0.8.0 (*)
      │       ├── chappe v0.1.0 (*)
      │       ├── fs-err v3.3.0
      │       │   └── axum-server v0.8.0 (*)
      │       ├── h2 v0.4.16
      │       │   └── hyper v1.9.0
      │       │       ├── axum v0.8.9 (*)
      │       │       ├── axum-server v0.8.0 (*)
      │       │       └── hyper-util v0.1.20
      │       │           ├── axum v0.8.9 (*)
      │       │           └── axum-server v0.8.0 (*)
      │       ├── hyper v1.9.0 (*)
      │       ├── hyper-util v0.1.20 (*)
      │       ├── marengo-deploy v0.1.0 (*)
      │       ├── marengo-gateway v0.1.0 (*)
      │       ├── marengo-jetson v0.1.0 (*)
      │       ├── marengo-pi v0.1.0 (*)
      │       ├── quinn v0.11.9 (*)
      │       ├── tokio-rustls v0.26.4 (*)
      │       ├── tokio-util v0.7.18
      │       │   ├── h2 v0.4.16 (*)
      │       │   ├── marengo-gateway v0.1.0 (*)
      │       │   └── tower-http v0.6.11 (*)
      │       ├── tower v0.5.3 (*)
      │       ├── tower-http v0.6.11 (*)
      │       ├── web-transport-proto v0.6.0 (*)
      │       └── web-transport-quinn v0.11.9 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── axum v0.8.9 (*)
      │       ├── axum-core v0.5.6
      │       │   └── axum v0.8.9 (*)
      │       ├── berthier v0.1.0 (*)
      │       ├── chappe v0.1.0 (*)
      │       ├── davout v0.1.0 (*)
      │       ├── fouche v0.1.0 (*)
      │       ├── h2 v0.4.16 (*)
      │       ├── imu-probe v0.1.0 (*)
      │       ├── marengo-deploy v0.1.0 (*)
      │       ├── marengo-gateway v0.1.0 (*)
      │       ├── marengo-imu v0.1.0 (*)
      │       ├── marengo-jetson v0.1.0 (*)
      │       ├── marengo-log-cli v0.1.0 (*)
      │       ├── marengo-pi v0.1.0 (*)
      │       ├── marengo-store v0.1.0 (*)
      │       ├── motor-repl v0.1.0 (*)
      │       ├── probe v0.1.0 (*)
      │       ├── quinn v0.11.9 (*)
      │       ├── quinn-proto v0.11.15 (*)
      │       ├── quinn-udp v0.5.14
      │       │   └── quinn v0.11.9 (*)
      │       ├── robstride v0.1.0 (*)
      │       ├── talleyrand v0.1.0 (*)
      │       ├── teleop v0.1.0 (*)
      │       ├── tower v0.5.3 (*)
      │       ├── tracing-subscriber v0.3.23 (*)
      │       ├── wave-demo v0.1.0 (*)
      │       └── web-transport-quinn v0.11.9 (*)
      ├── wasm-bindgen-macro-support v0.2.122
      │   └── wasm-bindgen-macro v0.2.122
      │       └── wasm-bindgen v0.2.122
      │           ├── getrandom v0.2.17 (*)
      │           ├── getrandom v0.3.4 (*)
      │           ├── iana-time-zone v0.1.65 (*)
      │           ├── js-sys v0.3.99 (*)
      │           └── web-time v1.1.0 (*)
      ├── windows-implement v0.60.2
      │   └── windows-core v0.62.2
      │       └── iana-time-zone v0.1.65 (*)
      ├── windows-interface v0.59.3
      │   └── windows-core v0.62.2 (*)
      ├── yoke-derive v0.8.2 (*)
      ├── zerofrom-derive v0.1.7 (*)
      └── zerovec-derive v0.11.3
          └── zerovec v0.11.6 (*)

warning[wildcard]: found 2 wildcard dependencies for crate 'talleyrand'
   ┌─ /Users/joseph/code/marengo-wt/audit/crates/talleyrand/Cargo.toml:14:30
   │
14 │ armee-kinematics.workspace = true
   │                              ━━━━ wildcard dependency
   │
   ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.toml:66:14
   │
66 │ talleyrand = { path = "crates/talleyrand" }
   │              ────────────────────────────── workspace dependency
   │
   ├ talleyrand v0.1.0
     └── marengo-jetson v0.1.0

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/teleop/Cargo.toml:18:29
   │
18 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ teleop v0.1.0

warning[wildcard]: found 1 wildcard dependency for crate 'teleop'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/teleop/Cargo.toml:18:29
   │
18 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
   │
   ├ teleop v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'thiserror'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:274:1
    │  
274 │ ╭ thiserror 1.0.69 registry+https://github.com/rust-lang/crates.io-index
275 │ │ thiserror 2.0.18 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ thiserror v1.0.69
      ├── armee-dynamics v0.1.0
      │   ├── (dev) berthier v0.1.0
      │   │   ├── marengo-pi v0.1.0
      │   │   ├── motor-repl v0.1.0
      │   │   └── wave-demo v0.1.0
      │   ├── marengo-log-cli v0.1.0
      │   ├── marengo-pi v0.1.0 (*)
      │   └── motor-repl v0.1.0 (*)
      ├── armee-kinematics v0.1.0
      │   ├── armee-dynamics v0.1.0 (*)
      │   ├── berthier v0.1.0 (*)
      │   ├── davout v0.1.0
      │   │   ├── berthier v0.1.0 (*)
      │   │   ├── marengo-pi v0.1.0 (*)
      │   │   └── motor-repl v0.1.0 (*)
      │   ├── marengo-config v0.1.0
      │   │   ├── (dev) armee-dynamics v0.1.0 (*)
      │   │   ├── (dev) armee-kinematics v0.1.0 (*)
      │   │   ├── berthier v0.1.0 (*)
      │   │   ├── davout v0.1.0 (*)
      │   │   ├── marengo-candump v0.1.0
      │   │   │   ├── marengo-gateway v0.1.0
      │   │   │   ├── marengo-log-cli v0.1.0 (*)
      │   │   │   └── marengo-store v0.1.0
      │   │   │       ├── marengo-gateway v0.1.0 (*)
      │   │   │       └── marengo-log-cli v0.1.0 (*)
      │   │   ├── marengo-gateway v0.1.0 (*)
      │   │   ├── marengo-homing v0.1.0
      │   │   │   ├── davout v0.1.0 (*)
      │   │   │   └── marengo-pi v0.1.0 (*)
      │   │   ├── marengo-limit-sync v0.1.0
      │   │   ├── marengo-log-cli v0.1.0 (*)
      │   │   ├── marengo-pi v0.1.0 (*)
      │   │   ├── motor-repl v0.1.0 (*)
      │   │   └── robstride v0.1.0
      │   │       ├── (dev) berthier v0.1.0 (*)
      │   │       ├── davout v0.1.0 (*)
      │   │       ├── marengo-candump v0.1.0 (*)
      │   │       ├── marengo-pi v0.1.0 (*)
      │   │       └── motor-repl v0.1.0 (*)
      │   ├── sim-harness v0.1.0
      │   └── talleyrand v0.1.0
      │       └── marengo-jetson v0.1.0
      ├── asn1-rs v0.6.2
      │   ├── der-parser v9.0.0
      │   │   └── x509-parser v0.16.0
      │   │       └── marengo-gateway v0.1.0 (*)
      │   ├── oid-registry v0.7.1
      │   │   └── x509-parser v0.16.0 (*)
      │   └── x509-parser v0.16.0 (*)
      ├── berthier v0.1.0 (*)
      ├── chappe v0.1.0
      │   ├── berthier v0.1.0 (*)
      │   ├── marengo-gateway v0.1.0 (*)
      │   ├── marengo-jetson v0.1.0 (*)
      │   └── marengo-pi v0.1.0 (*)
      ├── davout v0.1.0 (*)
      ├── fouche v0.1.0
      │   └── marengo-jetson v0.1.0 (*)
      ├── jni v0.21.1
      │   └── rustls-platform-verifier v0.6.2
      │       └── quinn-proto v0.11.15
      │           └── quinn v0.11.9
      │               ├── marengo-gateway v0.1.0 (*)
      │               └── web-transport-quinn v0.11.9
      │                   └── marengo-gateway v0.1.0 (*)
      ├── marengo-candump v0.1.0 (*)
      ├── marengo-config v0.1.0 (*)
      ├── marengo-deploy v0.1.0
      │   └── marengo-gateway v0.1.0 (*)
      ├── marengo-gateway v0.1.0 (*)
      ├── marengo-homing v0.1.0 (*)
      ├── marengo-host-metrics v0.1.0
      │   └── marengo-pi v0.1.0 (*)
      ├── marengo-imu v0.1.0
      │   └── imu-probe v0.1.0
      ├── marengo-log-cli v0.1.0 (*)
      ├── marengo-pi v0.1.0 (*)
      ├── marengo-store v0.1.0 (*)
      ├── robstride v0.1.0 (*)
      ├── talleyrand v0.1.0 (*)
      ├── urdf-rs v0.8.0
      │   ├── armee-dynamics v0.1.0 (*)
      │   ├── armee-kinematics v0.1.0 (*)
      │   ├── davout v0.1.0 (*)
      │   └── marengo-config v0.1.0 (*)
      └── x509-parser v0.16.0 (*)
    ├ thiserror v2.0.18
      ├── quinn v0.11.9
      │   ├── marengo-gateway v0.1.0
      │   └── web-transport-quinn v0.11.9
      │       └── marengo-gateway v0.1.0 (*)
      ├── quinn-proto v0.11.15
      │   └── quinn v0.11.9 (*)
      ├── web-transport-proto v0.6.0
      │   └── web-transport-quinn v0.11.9 (*)
      └── web-transport-quinn v0.11.9 (*)

warning[duplicate]: found 2 duplicate entries for crate 'thiserror-impl'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:276:1
    │  
276 │ ╭ thiserror-impl 1.0.69 registry+https://github.com/rust-lang/crates.io-index
277 │ │ thiserror-impl 2.0.18 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ thiserror-impl v1.0.69
      └── thiserror v1.0.69
          ├── armee-dynamics v0.1.0
          │   ├── (dev) berthier v0.1.0
          │   │   ├── marengo-pi v0.1.0
          │   │   ├── motor-repl v0.1.0
          │   │   └── wave-demo v0.1.0
          │   ├── marengo-log-cli v0.1.0
          │   ├── marengo-pi v0.1.0 (*)
          │   └── motor-repl v0.1.0 (*)
          ├── armee-kinematics v0.1.0
          │   ├── armee-dynamics v0.1.0 (*)
          │   ├── berthier v0.1.0 (*)
          │   ├── davout v0.1.0
          │   │   ├── berthier v0.1.0 (*)
          │   │   ├── marengo-pi v0.1.0 (*)
          │   │   └── motor-repl v0.1.0 (*)
          │   ├── marengo-config v0.1.0
          │   │   ├── (dev) armee-dynamics v0.1.0 (*)
          │   │   ├── (dev) armee-kinematics v0.1.0 (*)
          │   │   ├── berthier v0.1.0 (*)
          │   │   ├── davout v0.1.0 (*)
          │   │   ├── marengo-candump v0.1.0
          │   │   │   ├── marengo-gateway v0.1.0
          │   │   │   ├── marengo-log-cli v0.1.0 (*)
          │   │   │   └── marengo-store v0.1.0
          │   │   │       ├── marengo-gateway v0.1.0 (*)
          │   │   │       └── marengo-log-cli v0.1.0 (*)
          │   │   ├── marengo-gateway v0.1.0 (*)
          │   │   ├── marengo-homing v0.1.0
          │   │   │   ├── davout v0.1.0 (*)
          │   │   │   └── marengo-pi v0.1.0 (*)
          │   │   ├── marengo-limit-sync v0.1.0
          │   │   ├── marengo-log-cli v0.1.0 (*)
          │   │   ├── marengo-pi v0.1.0 (*)
          │   │   ├── motor-repl v0.1.0 (*)
          │   │   └── robstride v0.1.0
          │   │       ├── (dev) berthier v0.1.0 (*)
          │   │       ├── davout v0.1.0 (*)
          │   │       ├── marengo-candump v0.1.0 (*)
          │   │       ├── marengo-pi v0.1.0 (*)
          │   │       └── motor-repl v0.1.0 (*)
          │   ├── sim-harness v0.1.0
          │   └── talleyrand v0.1.0
          │       └── marengo-jetson v0.1.0
          ├── asn1-rs v0.6.2
          │   ├── der-parser v9.0.0
          │   │   └── x509-parser v0.16.0
          │   │       └── marengo-gateway v0.1.0 (*)
          │   ├── oid-registry v0.7.1
          │   │   └── x509-parser v0.16.0 (*)
          │   └── x509-parser v0.16.0 (*)
          ├── berthier v0.1.0 (*)
          ├── chappe v0.1.0
          │   ├── berthier v0.1.0 (*)
          │   ├── marengo-gateway v0.1.0 (*)
          │   ├── marengo-jetson v0.1.0 (*)
          │   └── marengo-pi v0.1.0 (*)
          ├── davout v0.1.0 (*)
          ├── fouche v0.1.0
          │   └── marengo-jetson v0.1.0 (*)
          ├── jni v0.21.1
          │   └── rustls-platform-verifier v0.6.2
          │       └── quinn-proto v0.11.15
          │           └── quinn v0.11.9
          │               ├── marengo-gateway v0.1.0 (*)
          │               └── web-transport-quinn v0.11.9
          │                   └── marengo-gateway v0.1.0 (*)
          ├── marengo-candump v0.1.0 (*)
          ├── marengo-config v0.1.0 (*)
          ├── marengo-deploy v0.1.0
          │   └── marengo-gateway v0.1.0 (*)
          ├── marengo-gateway v0.1.0 (*)
          ├── marengo-homing v0.1.0 (*)
          ├── marengo-host-metrics v0.1.0
          │   └── marengo-pi v0.1.0 (*)
          ├── marengo-imu v0.1.0
          │   └── imu-probe v0.1.0
          ├── marengo-log-cli v0.1.0 (*)
          ├── marengo-pi v0.1.0 (*)
          ├── marengo-store v0.1.0 (*)
          ├── robstride v0.1.0 (*)
          ├── talleyrand v0.1.0 (*)
          ├── urdf-rs v0.8.0
          │   ├── armee-dynamics v0.1.0 (*)
          │   ├── armee-kinematics v0.1.0 (*)
          │   ├── davout v0.1.0 (*)
          │   └── marengo-config v0.1.0 (*)
          └── x509-parser v0.16.0 (*)
    ├ thiserror-impl v2.0.18
      └── thiserror v2.0.18
          ├── quinn v0.11.9
          │   ├── marengo-gateway v0.1.0
          │   └── web-transport-quinn v0.11.9
          │       └── marengo-gateway v0.1.0 (*)
          ├── quinn-proto v0.11.15
          │   └── quinn v0.11.9 (*)
          ├── web-transport-proto v0.6.0
          │   └── web-transport-quinn v0.11.9 (*)
          └── web-transport-quinn v0.11.9 (*)

bug[unresolved-workspace-dependency]: failed to resolve a workspace dependency
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/wave-demo/Cargo.toml:19:29
   │
19 │ marengo-support.workspace = true
   │                             ━━━━
   │                             │
   │                             usage of workspace dependency
   │
   ├ wave-demo v0.1.0

warning[wildcard]: found 2 wildcard dependencies for crate 'wave-demo'
   ┌─ /Users/joseph/code/marengo-wt/audit/bins/wave-demo/Cargo.toml:18:12
   │
18 │ berthier = { path = "../../crates/berthier" }
   │            ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ wildcard dependency
19 │ marengo-support.workspace = true
   │                             ━━━━ wildcard dependency
   │
   ├ wave-demo v0.1.0

warning[duplicate]: found 3 duplicate entries for crate 'windows-sys'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:334:1
    │  
334 │ ╭ windows-sys 0.45.0 registry+https://github.com/rust-lang/crates.io-index
335 │ │ windows-sys 0.52.0 registry+https://github.com/rust-lang/crates.io-index
336 │ │ windows-sys 0.61.2 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows-sys v0.45.0
      └── jni v0.21.1
          └── rustls-platform-verifier v0.6.2
              └── quinn-proto v0.11.15
                  └── quinn v0.11.9
                      ├── marengo-gateway v0.1.0
                      └── web-transport-quinn v0.11.9
                          └── marengo-gateway v0.1.0 (*)
    ├ windows-sys v0.52.0
      ├── quinn-udp v0.5.14
      │   └── quinn v0.11.9
      │       ├── marengo-gateway v0.1.0
      │       └── web-transport-quinn v0.11.9
      │           └── marengo-gateway v0.1.0 (*)
      ├── ring v0.17.14
      │   ├── quinn-proto v0.11.15
      │   │   └── quinn v0.11.9 (*)
      │   ├── rcgen v0.13.2
      │   │   └── marengo-gateway v0.1.0 (*)
      │   ├── rustls v0.23.45
      │   │   ├── axum-server v0.8.0
      │   │   │   └── marengo-gateway v0.1.0 (*)
      │   │   ├── marengo-gateway v0.1.0 (*)
      │   │   ├── quinn v0.11.9 (*)
      │   │   ├── quinn-proto v0.11.15 (*)
      │   │   ├── rustls-platform-verifier v0.6.2
      │   │   │   └── quinn-proto v0.11.15 (*)
      │   │   ├── tokio-rustls v0.26.4
      │   │   │   ├── axum-server v0.8.0 (*)
      │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
      │   │   └── web-transport-quinn v0.11.9 (*)
      │   └── rustls-webpki v0.103.15
      │       ├── rustls v0.23.45 (*)
      │       └── rustls-platform-verifier v0.6.2 (*)
      └── socket2 v0.5.10
          ├── quinn v0.11.9 (*)
          └── quinn-udp v0.5.14 (*)
    ├ windows-sys v0.61.2
      ├── anstyle-query v1.1.5
      │   └── anstream v1.0.0
      │       └── clap_builder v4.6.0
      │           └── clap v4.6.1
      │               ├── marengo-limit-sync v0.1.0
      │               └── marengo-log-cli v0.1.0
      ├── anstyle-wincon v3.0.11
      │   └── anstream v1.0.0 (*)
      ├── ctrlc v3.5.2
      │   └── marengo-pi v0.1.0
      ├── errno v0.3.14
      │   ├── rustix v1.1.4
      │   │   └── tempfile v3.27.0
      │   │       ├── (dev) chappe v0.1.0
      │   │       │   ├── berthier v0.1.0
      │   │       │   │   ├── marengo-pi v0.1.0 (*)
      │   │       │   │   ├── motor-repl v0.1.0
      │   │       │   │   └── wave-demo v0.1.0
      │   │       │   ├── marengo-gateway v0.1.0
      │   │       │   ├── marengo-jetson v0.1.0
      │   │       │   └── marengo-pi v0.1.0 (*)
      │   │       ├── (dev) marengo-config v0.1.0
      │   │       │   ├── (dev) armee-dynamics v0.1.0
      │   │       │   │   ├── (dev) berthier v0.1.0 (*)
      │   │       │   │   ├── marengo-log-cli v0.1.0 (*)
      │   │       │   │   ├── marengo-pi v0.1.0 (*)
      │   │       │   │   └── motor-repl v0.1.0 (*)
      │   │       │   ├── (dev) armee-kinematics v0.1.0
      │   │       │   │   ├── armee-dynamics v0.1.0 (*)
      │   │       │   │   ├── berthier v0.1.0 (*)
      │   │       │   │   ├── davout v0.1.0
      │   │       │   │   │   ├── berthier v0.1.0 (*)
      │   │       │   │   │   ├── marengo-pi v0.1.0 (*)
      │   │       │   │   │   └── motor-repl v0.1.0 (*)
      │   │       │   │   ├── marengo-config v0.1.0 (*)
      │   │       │   │   ├── sim-harness v0.1.0
      │   │       │   │   └── talleyrand v0.1.0
      │   │       │   │       └── marengo-jetson v0.1.0 (*)
      │   │       │   ├── berthier v0.1.0 (*)
      │   │       │   ├── davout v0.1.0 (*)
      │   │       │   ├── marengo-candump v0.1.0
      │   │       │   │   ├── marengo-gateway v0.1.0 (*)
      │   │       │   │   ├── marengo-log-cli v0.1.0 (*)
      │   │       │   │   └── marengo-store v0.1.0
      │   │       │   │       ├── marengo-gateway v0.1.0 (*)
      │   │       │   │       └── marengo-log-cli v0.1.0 (*)
      │   │       │   ├── marengo-gateway v0.1.0 (*)
      │   │       │   ├── marengo-homing v0.1.0
      │   │       │   │   ├── davout v0.1.0 (*)
      │   │       │   │   └── marengo-pi v0.1.0 (*)
      │   │       │   ├── marengo-limit-sync v0.1.0 (*)
      │   │       │   ├── marengo-log-cli v0.1.0 (*)
      │   │       │   ├── marengo-pi v0.1.0 (*)
      │   │       │   ├── motor-repl v0.1.0 (*)
      │   │       │   └── robstride v0.1.0
      │   │       │       ├── (dev) berthier v0.1.0 (*)
      │   │       │       ├── davout v0.1.0 (*)
      │   │       │       ├── marengo-candump v0.1.0 (*)
      │   │       │       ├── marengo-pi v0.1.0 (*)
      │   │       │       └── motor-repl v0.1.0 (*)
      │   │       ├── (dev) marengo-deploy v0.1.0
      │   │       │   └── marengo-gateway v0.1.0 (*)
      │   │       ├── (dev) marengo-gateway v0.1.0 (*)
      │   │       ├── (dev) marengo-log-cli v0.1.0 (*)
      │   │       ├── (dev) marengo-pi v0.1.0 (*)
      │   │       ├── marengo-store v0.1.0 (*)
      │   │       ├── proptest v1.11.0
      │   │       │   └── (dev) berthier v0.1.0 (*)
      │   │       ├── prost-build v0.13.5
      │   │       │   └── (build) armee-proto v0.1.0
      │   │       │       ├── berthier v0.1.0 (*)
      │   │       │       ├── chappe v0.1.0 (*)
      │   │       │       ├── marengo-gateway v0.1.0 (*)
      │   │       │       ├── marengo-homing v0.1.0 (*)
      │   │       │       ├── marengo-host-metrics v0.1.0
      │   │       │       │   └── marengo-pi v0.1.0 (*)
      │   │       │       └── marengo-pi v0.1.0 (*)
      │   │       └── rusty-fork v0.3.1
      │   │           └── proptest v1.11.0 (*)
      │   └── signal-hook-registry v1.4.8
      │       ├── signal-hook v0.3.18
      │       │   └── marengo-pi v0.1.0 (*)
      │       └── tokio v1.52.3
      │           ├── axum v0.8.9
      │           │   └── marengo-gateway v0.1.0 (*)
      │           ├── axum-server v0.8.0
      │           │   └── marengo-gateway v0.1.0 (*)
      │           ├── chappe v0.1.0 (*)
      │           ├── fs-err v3.3.0
      │           │   └── axum-server v0.8.0 (*)
      │           ├── h2 v0.4.16
      │           │   └── hyper v1.9.0
      │           │       ├── axum v0.8.9 (*)
      │           │       ├── axum-server v0.8.0 (*)
      │           │       └── hyper-util v0.1.20
      │           │           ├── axum v0.8.9 (*)
      │           │           └── axum-server v0.8.0 (*)
      │           ├── hyper v1.9.0 (*)
      │           ├── hyper-util v0.1.20 (*)
      │           ├── marengo-deploy v0.1.0 (*)
      │           ├── marengo-gateway v0.1.0 (*)
      │           ├── marengo-jetson v0.1.0 (*)
      │           ├── marengo-pi v0.1.0 (*)
      │           ├── quinn v0.11.9
      │           │   ├── marengo-gateway v0.1.0 (*)
      │           │   └── web-transport-quinn v0.11.9
      │           │       └── marengo-gateway v0.1.0 (*)
      │           ├── tokio-rustls v0.26.4
      │           │   ├── axum-server v0.8.0 (*)
      │           │   └── (dev) marengo-gateway v0.1.0 (*)
      │           ├── tokio-util v0.7.18
      │           │   ├── h2 v0.4.16 (*)
      │           │   ├── marengo-gateway v0.1.0 (*)
      │           │   └── tower-http v0.6.11
      │           │       └── marengo-gateway v0.1.0 (*)
      │           ├── tower v0.5.3
      │           │   ├── axum v0.8.9 (*)
      │           │   └── (dev) marengo-gateway v0.1.0 (*)
      │           ├── tower-http v0.6.11 (*)
      │           ├── web-transport-proto v0.6.0
      │           │   └── web-transport-quinn v0.11.9 (*)
      │           └── web-transport-quinn v0.11.9 (*)
      ├── mio v1.2.0
      │   └── tokio v1.52.3 (*)
      ├── nu-ansi-term v0.50.3
      │   └── tracing-subscriber v0.3.23
      │       ├── chappe v0.1.0 (*)
      │       └── marengo-support v0.1.0
      │           ├── imu-probe v0.1.0
      │           ├── marengo-gateway v0.1.0 (*)
      │           ├── marengo-jetson v0.1.0 (*)
      │           ├── marengo-log-cli v0.1.0 (*)
      │           ├── marengo-pi v0.1.0 (*)
      │           ├── motor-repl v0.1.0 (*)
      │           ├── probe v0.1.0
      │           ├── teleop v0.1.0
      │           └── wave-demo v0.1.0 (*)
      ├── rustix v1.1.4 (*)
      ├── rustls-platform-verifier v0.6.2
      │   └── quinn-proto v0.11.15
      │       └── quinn v0.11.9 (*)
      ├── schannel v0.1.29
      │   └── rustls-native-certs v0.8.3
      │       ├── rustls-platform-verifier v0.6.2 (*)
      │       └── web-transport-quinn v0.11.9 (*)
      ├── socket2 v0.6.3
      │   └── tokio v1.52.3 (*)
      ├── tempfile v3.27.0 (*)
      ├── tokio v1.52.3 (*)
      └── winapi-util v0.1.11
          ├── same-file v1.0.6
          │   ├── davout v0.1.0 (*)
          │   ├── marengo-store v0.1.0 (*)
          │   └── walkdir v2.5.0
          │       └── (build) jni v0.21.1
          │           └── rustls-platform-verifier v0.6.2 (*)
          └── walkdir v2.5.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows-targets'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:337:1
    │  
337 │ ╭ windows-targets 0.42.2 registry+https://github.com/rust-lang/crates.io-index
338 │ │ windows-targets 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows-targets v0.42.2
      └── windows-sys v0.45.0
          └── jni v0.21.1
              └── rustls-platform-verifier v0.6.2
                  └── quinn-proto v0.11.15
                      └── quinn v0.11.9
                          ├── marengo-gateway v0.1.0
                          └── web-transport-quinn v0.11.9
                              └── marengo-gateway v0.1.0 (*)
    ├ windows-targets v0.52.6
      └── windows-sys v0.52.0
          ├── quinn-udp v0.5.14
          │   └── quinn v0.11.9
          │       ├── marengo-gateway v0.1.0
          │       └── web-transport-quinn v0.11.9
          │           └── marengo-gateway v0.1.0 (*)
          ├── ring v0.17.14
          │   ├── quinn-proto v0.11.15
          │   │   └── quinn v0.11.9 (*)
          │   ├── rcgen v0.13.2
          │   │   └── marengo-gateway v0.1.0 (*)
          │   ├── rustls v0.23.45
          │   │   ├── axum-server v0.8.0
          │   │   │   └── marengo-gateway v0.1.0 (*)
          │   │   ├── marengo-gateway v0.1.0 (*)
          │   │   ├── quinn v0.11.9 (*)
          │   │   ├── quinn-proto v0.11.15 (*)
          │   │   ├── rustls-platform-verifier v0.6.2
          │   │   │   └── quinn-proto v0.11.15 (*)
          │   │   ├── tokio-rustls v0.26.4
          │   │   │   ├── axum-server v0.8.0 (*)
          │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
          │   │   └── web-transport-quinn v0.11.9 (*)
          │   └── rustls-webpki v0.103.15
          │       ├── rustls v0.23.45 (*)
          │       └── rustls-platform-verifier v0.6.2 (*)
          └── socket2 v0.5.10
              ├── quinn v0.11.9 (*)
              └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_aarch64_gnullvm'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:339:1
    │  
339 │ ╭ windows_aarch64_gnullvm 0.42.2 registry+https://github.com/rust-lang/crates.io-index
340 │ │ windows_aarch64_gnullvm 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_aarch64_gnullvm v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_aarch64_gnullvm v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_aarch64_msvc'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:341:1
    │  
341 │ ╭ windows_aarch64_msvc 0.42.2 registry+https://github.com/rust-lang/crates.io-index
342 │ │ windows_aarch64_msvc 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰─────────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_aarch64_msvc v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_aarch64_msvc v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_i686_gnu'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:343:1
    │  
343 │ ╭ windows_i686_gnu 0.42.2 registry+https://github.com/rust-lang/crates.io-index
344 │ │ windows_i686_gnu 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰─────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_i686_gnu v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_i686_gnu v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_i686_msvc'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:346:1
    │  
346 │ ╭ windows_i686_msvc 0.42.2 registry+https://github.com/rust-lang/crates.io-index
347 │ │ windows_i686_msvc 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_i686_msvc v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_i686_msvc v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_x86_64_gnu'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:348:1
    │  
348 │ ╭ windows_x86_64_gnu 0.42.2 registry+https://github.com/rust-lang/crates.io-index
349 │ │ windows_x86_64_gnu 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_x86_64_gnu v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_x86_64_gnu v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_x86_64_gnullvm'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:350:1
    │  
350 │ ╭ windows_x86_64_gnullvm 0.42.2 registry+https://github.com/rust-lang/crates.io-index
351 │ │ windows_x86_64_gnullvm 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_x86_64_gnullvm v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_x86_64_gnullvm v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'windows_x86_64_msvc'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:352:1
    │  
352 │ ╭ windows_x86_64_msvc 0.42.2 registry+https://github.com/rust-lang/crates.io-index
353 │ │ windows_x86_64_msvc 0.52.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ windows_x86_64_msvc v0.42.2
      └── windows-targets v0.42.2
          └── windows-sys v0.45.0
              └── jni v0.21.1
                  └── rustls-platform-verifier v0.6.2
                      └── quinn-proto v0.11.15
                          └── quinn v0.11.9
                              ├── marengo-gateway v0.1.0
                              └── web-transport-quinn v0.11.9
                                  └── marengo-gateway v0.1.0 (*)
    ├ windows_x86_64_msvc v0.52.6
      └── windows-targets v0.52.6
          └── windows-sys v0.52.0
              ├── quinn-udp v0.5.14
              │   └── quinn v0.11.9
              │       ├── marengo-gateway v0.1.0
              │       └── web-transport-quinn v0.11.9
              │           └── marengo-gateway v0.1.0 (*)
              ├── ring v0.17.14
              │   ├── quinn-proto v0.11.15
              │   │   └── quinn v0.11.9 (*)
              │   ├── rcgen v0.13.2
              │   │   └── marengo-gateway v0.1.0 (*)
              │   ├── rustls v0.23.45
              │   │   ├── axum-server v0.8.0
              │   │   │   └── marengo-gateway v0.1.0 (*)
              │   │   ├── marengo-gateway v0.1.0 (*)
              │   │   ├── quinn v0.11.9 (*)
              │   │   ├── quinn-proto v0.11.15 (*)
              │   │   ├── rustls-platform-verifier v0.6.2
              │   │   │   └── quinn-proto v0.11.15 (*)
              │   │   ├── tokio-rustls v0.26.4
              │   │   │   ├── axum-server v0.8.0 (*)
              │   │   │   └── (dev) marengo-gateway v0.1.0 (*)
              │   │   └── web-transport-quinn v0.11.9 (*)
              │   └── rustls-webpki v0.103.15
              │       ├── rustls v0.23.45 (*)
              │       └── rustls-platform-verifier v0.6.2 (*)
              └── socket2 v0.5.10
                  ├── quinn v0.11.9 (*)
                  └── quinn-udp v0.5.14 (*)

warning[duplicate]: found 2 duplicate entries for crate 'wit-bindgen'
    ┌─ /Users/joseph/code/marengo-wt/audit/Cargo.lock:354:1
    │  
354 │ ╭ wit-bindgen 0.51.0 registry+https://github.com/rust-lang/crates.io-index
355 │ │ wit-bindgen 0.57.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ wit-bindgen v0.51.0
      └── wasip3 v0.4.0+wasi-0.3.0-rc-2026-01-06
          └── getrandom v0.4.2
              └── tempfile v3.27.0
                  ├── (dev) chappe v0.1.0
                  │   ├── berthier v0.1.0
                  │   │   ├── marengo-pi v0.1.0
                  │   │   ├── motor-repl v0.1.0
                  │   │   └── wave-demo v0.1.0
                  │   ├── marengo-gateway v0.1.0
                  │   ├── marengo-jetson v0.1.0
                  │   └── marengo-pi v0.1.0 (*)
                  ├── (dev) marengo-config v0.1.0
                  │   ├── (dev) armee-dynamics v0.1.0
                  │   │   ├── (dev) berthier v0.1.0 (*)
                  │   │   ├── marengo-log-cli v0.1.0
                  │   │   ├── marengo-pi v0.1.0 (*)
                  │   │   └── motor-repl v0.1.0 (*)
                  │   ├── (dev) armee-kinematics v0.1.0
                  │   │   ├── armee-dynamics v0.1.0 (*)
                  │   │   ├── berthier v0.1.0 (*)
                  │   │   ├── davout v0.1.0
                  │   │   │   ├── berthier v0.1.0 (*)
                  │   │   │   ├── marengo-pi v0.1.0 (*)
                  │   │   │   └── motor-repl v0.1.0 (*)
                  │   │   ├── marengo-config v0.1.0 (*)
                  │   │   ├── sim-harness v0.1.0
                  │   │   └── talleyrand v0.1.0
                  │   │       └── marengo-jetson v0.1.0 (*)
                  │   ├── berthier v0.1.0 (*)
                  │   ├── davout v0.1.0 (*)
                  │   ├── marengo-candump v0.1.0
                  │   │   ├── marengo-gateway v0.1.0 (*)
                  │   │   ├── marengo-log-cli v0.1.0 (*)
                  │   │   └── marengo-store v0.1.0
                  │   │       ├── marengo-gateway v0.1.0 (*)
                  │   │       └── marengo-log-cli v0.1.0 (*)
                  │   ├── marengo-gateway v0.1.0 (*)
                  │   ├── marengo-homing v0.1.0
                  │   │   ├── davout v0.1.0 (*)
                  │   │   └── marengo-pi v0.1.0 (*)
                  │   ├── marengo-limit-sync v0.1.0
                  │   ├── marengo-log-cli v0.1.0 (*)
                  │   ├── marengo-pi v0.1.0 (*)
                  │   ├── motor-repl v0.1.0 (*)
                  │   └── robstride v0.1.0
                  │       ├── (dev) berthier v0.1.0 (*)
                  │       ├── davout v0.1.0 (*)
                  │       ├── marengo-candump v0.1.0 (*)
                  │       ├── marengo-pi v0.1.0 (*)
                  │       └── motor-repl v0.1.0 (*)
                  ├── (dev) marengo-deploy v0.1.0
                  │   └── marengo-gateway v0.1.0 (*)
                  ├── (dev) marengo-gateway v0.1.0 (*)
                  ├── (dev) marengo-log-cli v0.1.0 (*)
                  ├── (dev) marengo-pi v0.1.0 (*)
                  ├── marengo-store v0.1.0 (*)
                  ├── proptest v1.11.0
                  │   └── (dev) berthier v0.1.0 (*)
                  ├── prost-build v0.13.5
                  │   └── (build) armee-proto v0.1.0
                  │       ├── berthier v0.1.0 (*)
                  │       ├── chappe v0.1.0 (*)
                  │       ├── marengo-gateway v0.1.0 (*)
                  │       ├── marengo-homing v0.1.0 (*)
                  │       ├── marengo-host-metrics v0.1.0
                  │       │   └── marengo-pi v0.1.0 (*)
                  │       └── marengo-pi v0.1.0 (*)
                  └── rusty-fork v0.3.1
                      └── proptest v1.11.0 (*)
    ├ wit-bindgen v0.57.1
      └── wasip2 v1.0.3+wasi-0.2.9
          ├── getrandom v0.3.4
          │   ├── fastbloom v0.14.1
          │   │   └── quinn-proto v0.11.15
          │   │       └── quinn v0.11.9
          │   │           ├── marengo-gateway v0.1.0
          │   │           └── web-transport-quinn v0.11.9
          │   │               └── marengo-gateway v0.1.0 (*)
          │   ├── jobserver v0.1.34
          │   │   └── cc v1.2.62
          │   │       ├── (build) aws-lc-sys v0.45.0
          │   │       │   └── aws-lc-rs v1.18.1
          │   │       │       ├── quinn-proto v0.11.15 (*)
          │   │       │       ├── rustls v0.23.45
          │   │       │       │   ├── axum-server v0.8.0
          │   │       │       │   │   └── marengo-gateway v0.1.0 (*)
          │   │       │       │   ├── marengo-gateway v0.1.0 (*)
          │   │       │       │   ├── quinn v0.11.9 (*)
          │   │       │       │   ├── quinn-proto v0.11.15 (*)
          │   │       │       │   ├── rustls-platform-verifier v0.6.2
          │   │       │       │   │   └── quinn-proto v0.11.15 (*)
          │   │       │       │   ├── tokio-rustls v0.26.4
          │   │       │       │   │   ├── axum-server v0.8.0 (*)
          │   │       │       │   │   └── (dev) marengo-gateway v0.1.0 (*)
          │   │       │       │   └── web-transport-quinn v0.11.9 (*)
          │   │       │       └── rustls-webpki v0.103.15
          │   │       │           ├── rustls v0.23.45 (*)
          │   │       │           └── rustls-platform-verifier v0.6.2 (*)
          │   │       ├── cmake v0.1.58
          │   │       │   └── (build) aws-lc-sys v0.45.0 (*)
          │   │       ├── (build) iana-time-zone-haiku v0.1.2
          │   │       │   └── iana-time-zone v0.1.65
          │   │       │       └── chrono v0.4.44
          │   │       │           └── marengo-homing v0.1.0
          │   │       │               ├── davout v0.1.0
          │   │       │               │   ├── berthier v0.1.0
          │   │       │               │   │   ├── marengo-pi v0.1.0
          │   │       │               │   │   ├── motor-repl v0.1.0
          │   │       │               │   │   └── wave-demo v0.1.0
          │   │       │               │   ├── marengo-pi v0.1.0 (*)
          │   │       │               │   └── motor-repl v0.1.0 (*)
          │   │       │               └── marengo-pi v0.1.0 (*)
          │   │       ├── (build) libsqlite3-sys v0.30.1
          │   │       │   └── rusqlite v0.32.1
          │   │       │       ├── davout v0.1.0 (*)
          │   │       │       └── marengo-store v0.1.0
          │   │       │           ├── marengo-gateway v0.1.0 (*)
          │   │       │           └── marengo-log-cli v0.1.0
          │   │       └── (build) ring v0.17.14
          │   │           ├── quinn-proto v0.11.15 (*)
          │   │           ├── rcgen v0.13.2
          │   │           │   └── marengo-gateway v0.1.0 (*)
          │   │           ├── rustls v0.23.45 (*)
          │   │           └── rustls-webpki v0.103.15 (*)
          │   ├── quinn-proto v0.11.15 (*)
          │   └── rand_core v0.9.5
          │       ├── rand v0.9.4
          │       │   ├── fastbloom v0.14.1 (*)
          │       │   ├── proptest v1.11.0
          │       │   │   └── (dev) berthier v0.1.0 (*)
          │       │   └── quinn-proto v0.11.15 (*)
          │       ├── rand_chacha v0.9.0
          │       │   ├── proptest v1.11.0 (*)
          │       │   └── rand v0.9.4 (*)
          │       └── rand_xorshift v0.4.0
          │           └── proptest v1.11.0 (*)
          └── getrandom v0.4.2
              └── tempfile v3.27.0
                  ├── (dev) chappe v0.1.0
                  │   ├── berthier v0.1.0 (*)
                  │   ├── marengo-gateway v0.1.0 (*)
                  │   ├── marengo-jetson v0.1.0
                  │   └── marengo-pi v0.1.0 (*)
                  ├── (dev) marengo-config v0.1.0
                  │   ├── (dev) armee-dynamics v0.1.0
                  │   │   ├── (dev) berthier v0.1.0 (*)
                  │   │   ├── marengo-log-cli v0.1.0 (*)
                  │   │   ├── marengo-pi v0.1.0 (*)
                  │   │   └── motor-repl v0.1.0 (*)
                  │   ├── (dev) armee-kinematics v0.1.0
                  │   │   ├── armee-dynamics v0.1.0 (*)
                  │   │   ├── berthier v0.1.0 (*)
                  │   │   ├── davout v0.1.0 (*)
                  │   │   ├── marengo-config v0.1.0 (*)
                  │   │   ├── sim-harness v0.1.0
                  │   │   └── talleyrand v0.1.0
                  │   │       └── marengo-jetson v0.1.0 (*)
                  │   ├── berthier v0.1.0 (*)
                  │   ├── davout v0.1.0 (*)
                  │   ├── marengo-candump v0.1.0
                  │   │   ├── marengo-gateway v0.1.0 (*)
                  │   │   ├── marengo-log-cli v0.1.0 (*)
                  │   │   └── marengo-store v0.1.0 (*)
                  │   ├── marengo-gateway v0.1.0 (*)
                  │   ├── marengo-homing v0.1.0 (*)
                  │   ├── marengo-limit-sync v0.1.0
                  │   ├── marengo-log-cli v0.1.0 (*)
                  │   ├── marengo-pi v0.1.0 (*)
                  │   ├── motor-repl v0.1.0 (*)
                  │   └── robstride v0.1.0
                  │       ├── (dev) berthier v0.1.0 (*)
                  │       ├── davout v0.1.0 (*)
                  │       ├── marengo-candump v0.1.0 (*)
                  │       ├── marengo-pi v0.1.0 (*)
                  │       └── motor-repl v0.1.0 (*)
                  ├── (dev) marengo-deploy v0.1.0
                  │   └── marengo-gateway v0.1.0 (*)
                  ├── (dev) marengo-gateway v0.1.0 (*)
                  ├── (dev) marengo-log-cli v0.1.0 (*)
                  ├── (dev) marengo-pi v0.1.0 (*)
                  ├── marengo-store v0.1.0 (*)
                  ├── proptest v1.11.0 (*)
                  ├── prost-build v0.13.5
                  │   └── (build) armee-proto v0.1.0
                  │       ├── berthier v0.1.0 (*)
                  │       ├── chappe v0.1.0 (*)
                  │       ├── marengo-gateway v0.1.0 (*)
                  │       ├── marengo-homing v0.1.0 (*)
                  │       ├── marengo-host-metrics v0.1.0
                  │       │   └── marengo-pi v0.1.0 (*)
                  │       └── marengo-pi v0.1.0 (*)
                  └── rusty-fork v0.3.1
                      └── proptest v1.11.0 (*)

advisories ok, bans ok, licenses ok, sources ok

```

## cargo audit — **FAIL or unknown**

```
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1290 security advisories (from /Users/joseph/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (400 crate dependencies)
Crate:     paste
Version:   1.0.15
Warning:   unmaintained
Title:     paste - no longer maintained
Date:      2024-10-07
ID:        RUSTSEC-2024-0436
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0436

warning: 1 allowed warning found

```