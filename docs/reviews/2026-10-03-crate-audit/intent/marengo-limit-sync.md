# Intent card — `marengo-limit-sync`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-limit-sync` |
| Path | `bins/marengo-limit-sync` |
| Kind | bin (`src/main.rs`); no features (`Cargo.toml:13-20`) |
| Baseline | `a2b55b3` |
| LOC | src 71 / tests 0 in crate (`metrics/loc.md:25`). Cargo coverage 0 % (`metrics/coverage-by-crate.md:14`, `coverage-by-file.md:11`) because its only tests are Node HTTP tests that build and run the real CLI (`tools/limit-sync-local/test/server.test.mjs:46-49,264,280`) |
| Sources | `src/main.rs:1-3`, `Cargo.toml:4`, ADR 0012, ADR 0017 §4 (`0017:18`), ADR 0032, `docs/safety.md:95`, `tools/limit-sync-local/{README.md,server.ts}`, `justfile:70-73`, Consul `src/lib/persist-joint-limits.ts:60-198`, `src/components/dashboard/hardware/local-limit-sync-session.tsx:12`, prior `2026-09-29/tooling.md` T03/T04, `batch30-local-writer-session-auth.md`, ledger; `git log -- bins/marengo-limit-sync tools/limit-sync-local` (`f743825` 2026-07-22 "expand URDF on Set Limits and wait for Durable persist" created it; `17b4d7a` stop probing unless configured; `a5c4cdb` 2026-10-02 PR244 session credential + bounded work + checkout-bound writes) |

## 2. Intent

A **workstation-only mirror** for the Live limit patch: after the Pi reports **Durable** for a Set Limits apply, it writes the same taught hard bounds, ADR 0009-inset soft bounds and expand-only URDF into the operator's **local git checkout**, so the repository does not drift behind the Pi (`main.rs:1-3`; ADR 0017 §4 "optional Durable-gated sync via `marengo-limit-sync` + loopback `tools/limit-sync-local` — same Rust helpers, never on Pending alone"). ADR 0032 binds it to an explicit checkout root so runtime resolvers (`MARENGO_CONFIG_DIR`, `/opt/marengo/config`) cannot redirect the write. It is a CLI veneer: all validation and writing live in `marengo_config::apply_local_limit_patch` (`crates/marengo-config/src/urdf_expand.rs:136-176`). It does not establish a Pi generation transaction or multi-file durability (ADR 0032 ¶4).

Conflicting statements: `main.rs:3` "Invoked by Consul" — Consul never spawns it; the loopback Node server `tools/limit-sync-local/server.ts:94-98` does, after Consul POSTs `/local/limit-patch` (`persist-joint-limits.ts:177`). `docs/safety.md:95` "Local git sync is Durable-gated via `marengo-limit-sync` only": the Durable gate lives in Consul (`persist-joint-limits.ts:126`), not in this binary, which accepts any invocation.

## 3. Owns / Must not

| Owns | Must not |
|---|---|
| Parse `--repo-root --joint --lower --upper [--soft-inset] [--soft-lower --soft-upper]` and build a `LimitPatch` (`main.rs:17-55`) | Write the Pi or installed tree (ADR 0032 ¶5 "No test may redirect writes into the robot's installed tree") — upheld: only `--repo-root/config` (`urdf_expand.rs:141`) |
| Exit status/receipt for the Node server (`main.rs:58-70`) | Decide Durable gating (Consul owns it; `persist-joint-limits.ts:126`) |
| | Implement limit math (owned by `marengo-config`, `limit_patch.rs:13-34`) — upheld |
| | `bins` rule "Use `marengo_support::init_tracing()`" (root AGENTS rule; `bins/AGENTS.md` conventions) — **not followed**: uses `eprintln!` only (`main.rs:60-68`); harmless for a one-shot CLI |

## 4. Interface

| Surface | Code | Consumer |
|---|---|---|
| CLI args | `main.rs:14-39` | `tools/limit-sync-local/server.ts:83-91` passes `--repo-root --joint --lower --upper` and, only when both are valid, `--soft-lower --soft-upper`; never `--soft-inset` |
| Binary path | default `target/debug/marengo-limit-sync` or `MARENGO_LIMIT_SYNC_BIN` (`server.ts:95-96`) | `justfile:71-73` builds it before starting the server |
| Exit 0/1 + one stderr line | `main.rs:58-70` | `server.ts:98-120` (bounded output, 5 s deadline per README) |

Chain: Consul Set Limits → gateway `/config/patch` (Durable) → Consul `defaultLocalLimitSync` (`persist-joint-limits.ts:158-198`, only if `VITE_LIMIT_SYNC_URL` set and a local credential entered) → loopback server (Origin + Bearer + JSON, `README.md:14-25`) → this CLI → `marengo_config::apply_local_limit_patch`.

Depth: shallow pass-through (one function call). No seam of its own. Unused dependency `anyhow` (`Cargo.toml:18`; `metrics/unused-deps.md:29-30`; 0 uses in `src/`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| Negative bounds parse as values, not flags | `allow_negative_numbers` `main.rs:25,28,34,37` | `server.test.mjs:264` (real CLI) |
| Writes bind to the supplied checkout, ignoring `MARENGO_CONFIG_DIR` | delegated: `urdf_expand.rs:140-176` (ADR 0032) | `server.test.mjs:264,280` |
| Soft defaults to hard ± inset when explicit soft not given as a pair | `main.rs:43-46` → `soft_limits_with_inset` (`limit_patch.rs:13-20`) | none in Rust (crate unit `limit_patch.rs:229,236` cover the helper) |
| Patch validated before any write (finite, ordered, soft within hard) | delegated `validate_limit_patch` (`urdf_expand.rs:149`) | `server.test.mjs:219` (zero-width refused, but at server layer) |
| Durable-only invocation | **not owned here**; Consul `persist-joint-limits.ts:126` | `consul/src/lib/persist-joint-limits.test.ts:32,57` |

## 6. Inputs / outputs

- **Args** only; no env read by the binary itself (`main.rs`). `server.ts` uses `LIMIT_SYNC_PORT`, `LIMIT_SYNC_TOKEN`, `MARENGO_LIMIT_SYNC_BIN` (`README.md:10-13`).
- **Files read/written**: `<repo-root>/config/motors.yaml`, `<repo-root>/config/control.yaml`, URDF resolved from that config/root (`urdf_expand.rs:141-175`; ADR 0032 ¶2).
- **Output**: stderr one line; exit code. No Chappe, CAN, HTTP, DB.

## 7. Prior review reconciliation

| Id | Prior | Current status | Evidence |
|---|---|---|---|
| T04 | loopback writer accepts unauthenticated foreign-origin writes, unbounded body/child | **fixed in code** (`a5c4cdb`, PR244); ledger status `partial` | `batch30-local-writer-session-auth.md:8-25`; `server.ts:134,145`; tests `server.test.mjs:112,162,182,206,295,306` |
| (batch30) | negative args rejected; config redirection | fixed | `main.rs:25-37`; ADR 0032; `server.test.mjs:264,280` |
| T03 | config/URDF sync bypasses Pi transaction | open (concerns MCP Pi sync, not this local mirror) | ledger `open`; ADR 0032 ¶4 keeps generation transaction out of scope |

## 8. Drift

- Not listed in `bins/AGENTS.md:3-19` ("9 binaries" table) nor `bins/codemap.md:7-17`; no `codemap.md` in `bins/marengo-limit-sync/`.
- `main.rs:3` "Invoked by Consul": invoked by `tools/limit-sync-local/server.ts:98`.
- `Cargo.toml:4` and `main.rs:1` say "motors + control soft + expand-only URDF" — accurate; but Consul still sends a `profile` field (`persist-joint-limits.ts:185`) that neither server nor CLI uses (bringup profiles retired, `CONTEXT.md:38`).

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Deleting touches |
|---|---|---|---|
| `anyhow` dependency (`Cargo.toml:18`) | zero references (`metrics/unused-deps.md:29-30`) | high | `Cargo.toml`, `Cargo.lock` |
| `ensure_soft_inset(&mut patch)` call (`main.rs:56`) | duplicate: both soft fields are already `Some` (`main.rs:52-53`) and `apply_local_limit_patch` calls it again (`urdf_expand.rs:151`) | high | `main.rs` imports |
| `--soft-inset` flag (`main.rs:30-32`) | never passed by the only consumer (`server.ts:83-91`) | low | CLI surface only; keep if manual use is intended |
| Consul `profile` body field to the local server (`persist-joint-limits.ts:185`) | superseded (bringup profiles retired) | low | Consul only |

The binary itself is in use (single consumer chain, ADR 0017/0032); not a prune candidate.

## 10. Phase-B leads

1. **Mirrors requested, not applied, values**: Consul sends its own `hardLower/hardUpper/softLower/softUpper` (`persist-joint-limits.ts:126-135`), not the Pi's durable receipt; the gateway reply carries no limits (`bins/marengo-gateway/src/config.rs:247-257`). If the Pi clamps or expands differently, the checkout silently diverges.
2. **One-sided soft bound silently dropped**: if only `--soft-lower` or only `--soft-upper` is given, both are ignored and inset defaults are used (`main.rs:43-46`) without error. The server prevents this (`server.ts:84-91`); direct CLI users are not protected.
3. **Torque and velocity never mirrored** (`main.rs:51,54` force `None`) while the Live limit patch surface includes them (`docs/safety.md:90`; gateway `config.rs:67-70`). Today Consul Set Limits sends only positions (`persist-joint-limits.ts:96-101`), so this is latent.
4. **No revision/generation check** between checkout and Pi (ADR 0032 ¶4 acknowledges); a checkout on another branch receives the patch.
5. **Durable gate lives only in the browser**: the CLI and server accept any authenticated request regardless of Pi state (`server.ts:134-156`), contrary to the wording of `docs/safety.md:95`.
6. **Zero Rust coverage** (`coverage-by-file.md:11`): the Node suite is the only guard; any Rust-only CI lane would not exercise this binary.
