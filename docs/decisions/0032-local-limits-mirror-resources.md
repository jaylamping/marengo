# ADR 0032: bind the local limits mirror to its checkout

Status: accepted for software implementation, October 2, 2026.

The loopback Set Limits writer mirrors values after the Pi reports durable
acceptance. It starts the Rust CLI with an explicit repository root. Reusing the
runtime config resolver could select `MARENGO_CONFIG_DIR` or `/opt/marengo/config`
instead, mixing another configuration's YAML with the requested checkout's URDF.
An isolated two-checkout HTTP test reproduces this redirection.

`apply_local_limit_patch` loads and writes the supplied root's `config/` tree.
The URDF remains resolved from that configuration and root. Ordinary runtime
loaders retain their existing environment and installation precedence. The CLI
accepts negative numeric hard and soft bounds as separate arguments, matching
the HTTP writer's argument vector and valid joint ranges.

The listener requires an approved local Origin, JSON and a runtime session
credential before admitting bounded work. It invokes one asynchronous Rust
writer at a time. The credential remains in the owner terminal and the Consul
tab's memory; it is not supplied through Vite or browser storage. Automated tests
use disposable checkout copies and stand-in workers for timeout/output cases.

Mirroring the accepted values does not establish a Pi generation transaction,
multi-file power-loss durability or physical commissioning. Those remain T03
and the reference/deployment work. No test may redirect writes into the robot's
installed tree or authorize motors.

Implementation amendment (2026-10-03): profile updates use a stable SHA-256
revision over canonical YAML and atomic per-file replacement under the shared
profile lock. This does not make the multi-file mirror crash-atomic, nor does
the checkout writer verify that its starting revision matches the Pi's revision;
those remain explicit limitations.
