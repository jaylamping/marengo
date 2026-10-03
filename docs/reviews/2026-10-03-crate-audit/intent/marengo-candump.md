# Intent card: `marengo-candump`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-candump` (candump inspection deep module) |
| Path | `crates/marengo-candump` |
| Kind | lib. Feature `robstride-enrichment = ["dep:robstride", "dep:marengo-config"]`, off by default (`Cargo.toml:13-15`); the workspace pins `default-features = false` (root `Cargo.toml:72`) |
| Baseline | `a2b55b3` (worktree `audit/2026-10-03`) |
| LOC | src 1,007 (`lib.rs` 455, of which 24 are inline tests; `scan.rs` 487; `robstride.rs` 65). tests/ 449 (`contract.rs` 253, `input_validation.rs` 101, `input_limits.rs` 95) plus 7 fixtures. metrics/loc.md:17 |
| Coverage | crate 75.1% lines (metrics/coverage-by-crate.md:23). `lib.rs` 78.6%, `scan.rs` 80.7%, **`robstride.rs` 0.0% (0/37)** (metrics/coverage-by-file.md:15,39,45) |
| Sources consulted | `src/lib.rs:1-33` //! docs; `Cargo.toml`; ADR 0001:32-41; ADR 0011:8-22; ADR 0028:28; ADR 0030:28; ADR 0036:176; docs/logging-taxonomy.md:11,62; bins/marengo-log-cli/codemap.md:5,12,18; prior review gateway.md (G16, G17, design note 12), tooling.md T14, batch18-candump-input-validation.md, finding-index.md, implementation-ledger.json (G17, T14); `git log -- crates/marengo-candump` (12 commits, first `24db6c6` 2026-07-23); `cargo tree -i marengo-candump`; consumer greps across crates/, bins/, tools/, consul/src, scripts/; `cargo check -p marengo-candump --features robstride-enrichment --all-targets` (clean); metrics/*.md |

## 2. Intent

marengo-candump exists so that every reader of a recorded `candump` capture parses it the same way. Readers are the session archive (marengo-store), operator CLI (marengo-log-cli), gateway HTTP, and through them Consul and the MCP `pi_candump_summary`. It is the single seam for: parsing (can-utils log `ID#HEX` and default ASCII `ID [dlc] XX…`), timestamp normalization, gzip detection, bounded streaming, counts and rates, deterministic top IDs, parsed-frame paging, and optional Robstride labelling (`lib.rs:1-19`, `24db6c6` "add deep module with inspect seam", `5deae83` "delegate candump to deep module"; ADR 0011:13 "candump parsing via `marengo-candump` (no frame-index table)").

Candump files are the bench's **wire truth versus code trace** (docs/logging-taxonomy.md:62). The crate turns them into summaries (`parsed_frames`, Hz, top IDs) that operators and agents use to check motion runs against the bus.

Since `5084ad0`/`085d701` (G17) it also treats captures as **untrusted input**:
- lines are capped at 4,096 bytes;
- decompressed input is capped at 256 MiB;
- out-of-range timestamps are a typed error, not a panic (`lib.rs:14-19`).

It deliberately exposes no public line parser, so callers cannot fork the logic (`lib.rs:25`).

Conflicting statements of intent:
- `lib.rs:4-5` promises "optional Robstride enrichment" with "direction-aware device_id" (`lib.rs:172`). The enrichment reuses `robstride::comm::inbound_motor_device_id`. That function is direction-aware only for status, fault and report frames. Its own docs (`robstride/src/comm.rs:49-51`) say type-0 and type-17 replies need the identity and params decoders, which candump does not use (lead L3).
- ADR 0001:39 says gateway and store paths "always inspect as Delta" and the CLI uses Absolute "when requested". The enum's name `Delta` actually means zero-based `candump -t z` (`lib.rs:55`), not can-utils `-t d` (inter-frame delta). Feeding a real `-t d` log produces `TimestampRegression` errors (`scan.rs:202-210`).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| Line syntax (log and ASCII forms, DLC token, hex payload ≤8 bytes, DLC must match payload) | `scan.rs:337-433` |
| Timestamp domain: monotonic, Delta fits `Duration`, Absolute fits under 2^64 µs | `scan.rs:202-249` |
| Input bounds: 4,096 B per line, 256 MiB decompressed, gzip by magic bytes | `scan.rs:19-22,63-158` |
| Summary: total/parsed counts, duration, per-interface Hz, top IDs sorted by count desc then id asc, limit ≤64 | `scan.rs:279-327`; `lib.rs:191-207,249-290` |
| Parsed-frame paging, limit 1..=5000 | `lib.rs:209-240`; `scan.rs:259-274` |
| Validated 29-bit `CanId`, canonical hex serde | `lib.rs:61-111` |
| Text formatter for CLI `--format text` | `lib.rs:355-430` |
| Optional Robstride labels (`comm_type`, name, inbound device id, joint) from motors.yaml | `robstride.rs:13-65`; `scan.rs:435-487` |

| Must not (source) | Status |
|---|---|
| Own session/blob path lookup (`lib.rs:23`) | Respected. Paths are supplied by store, CLI and gateway |
| Depend on SQLite or gateway HTTP (`lib.rs:24`) | Respected. Dependencies are flate2, serde, thiserror, plus optional robstride/marengo-config (`Cargo.toml:17-22`) |
| Expose a public line parser (`lib.rs:25`) | Respected. `parse_frame_fields` is private (`scan.rs:337`) |
| Re-implement the Robstride wire format (AGENTS.md:199 spirit) | **Partly.** It reuses `unpack_ext_id`, `CommunicationType` and `inbound_motor_device_id` (`scan.rs:459-462`). It re-implements the comm-type label table (`scan.rs:473-487`), a private `MotorAddress` type (`robstride.rs:7-11`, duplicating `robstride::MotorAddress` at `robstride/src/bus.rs:89`), and duplicate-address catalog validation (`robstride.rs:35-64`, duplicating `SocketCanRouter::open` at `robstride/src/bus.rs:1394-1407`) |

## 4. Interface

| Group | Key items | Consumers |
|---|---|---|
| Facade | `Candump::{plain, default, inspect_path, inspect_bytes}`; feature-gated `with_robstride`, `with_robstride_from_config_dir`, `MotorCatalog` | marengo-store `store.rs:24,35,41,477,555-640` (`plain`, injected); marengo-gateway `main.rs:130-145` (`with_robstride_from_config_dir`, falls back to plain); marengo-log-cli `main.rs:188-205,244-248` |
| Request | `InspectRequest::{summary, page, with_top_id_limit}`, `FramePage::new` (`MAX_LIMIT=5000`), `TimestampMode::{Delta, Absolute}` | store (always `Delta`: `store.rs:477,570,598,613,635`); log-cli (`--timestamp` required, `main.rs:158,173-180,207-218`). `with_top_id_limit` is used only in `contract.rs:178` (metrics/pub-usage.md:51) |
| Result model (serde) | `Inspection`, `Summary`, `InterfaceSummary`, `CanIdCount`, `Frame`, `FrameEnrichment`, `CanId`, `UnixMicros`, `Error` | store re-exports `Frame`/`Summary` (`marengo-store/src/lib.rs:23`) and wraps `Error` (`error.rs:10`); gateway re-maps to its own JSON (`logs.rs:136-200`) with one-release aliases (ADR 0001:36); log-cli prints serde JSON directly (`main.rs:233-236`) |
| Text | `format_inspection_text` | log-cli `main.rs:238` |
| Indirect | gateway HTTP `GET /logs/sessions/{id}/candump`, `/candump/summary`, `/logs/sessions/latest/candump[/summary]` (`http.rs:109-116`) → Consul `hooks/use-candump-data.ts`, `lib/can-traffic-spectrum.ts:53,168` (reads `comm_type_name`; does not re-decode). MCP `pi_candump_summary` and `scripts/pi-remote.sh:115` run `marengo-log-cli candump summary --timestamp delta --format json` (`tools/marengo-pi-mcp/src/tools/logs.ts:132`) |

**Depth.** The module is genuinely deep: three entry points over about 1k LOC that hide parsing, bounds, gzip and paging, with an enum-private enrichment mode instead of a trait (`lib.rs:300-304`). There is no trait seam and no adapter count to report. Enrichment is a private closed enum (`scan.rs:55-61`). The facade's leaky part is the serde result types, which the gateway re-shapes by hand (`logs.rs:136-200`); ADR 0001:34-39 records that as a deliberate HTTP/proto divergence (`proto/marengo/v1/marengo.proto:439-484`).

**Overlap.**
- *With marengo-log-cli:* none in logic. The CLI is a thin clap wrapper (bins/marengo-log-cli/codemap.md:5,12). Its own feature `robstride-enrichment` forwards to this crate (`bins/marengo-log-cli/Cargo.toml:19`).
- *With robstride decoding:* candump labels IDs only. Robstride decodes payloads. The overlap is the label table and the address and catalog types (§3).
- *With marengo-gateway:* `candump_*_json` duplicates the serde model, adds alias fields and drops `enrichment.device_id` (`logs.rs:138-145`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) | File coverage |
|---|---|---|---|
| `CanId` ≤ 0x1FFF_FFFF, including when deserialized | `lib.rs:69-74,102-111` | `lib.rs:438,443`; `contract.rs:224` | lib 78.6% |
| Physical line ≤4096 B including newline, checked before allocation | `scan.rs:126-148` | `input_limits.rs:8`; `input_validation.rs:70` | scan 80.7% |
| Decompressed capture ≤256 MiB | `scan.rs:126-143` | `input_limits.rs:42` | 80.7% |
| Finite out-of-domain timestamps → typed error, no panic (G17) | `scan.rs:212-247` | `input_validation.rs:8`; `input_limits.rs:69` | 80.7% |
| JSON offsets finite, non-negative, fit `Duration` | `lib.rs:134-145` | `input_validation.rs:36` | 78.6% |
| ASCII DLC ≤8 and equal to payload byte count; odd hex rejected | `scan.rs:374-398` | `input_validation.rs:49,81` | 80.7% |
| Timestamps never regress (whole inspection fails) | `scan.rs:202-210` | `contract.rs:191` | 80.7% |
| Malformed lines count in `total_lines` but never in frames; page indexes parsed frames | `scan.rs:191-200,251-274` | `contract.rs:114,208` | 80.7% |
| Truncated or corrupt gzip → I/O error | `scan.rs:127-134` | `input_limits.rs:57,85`; `contract.rs:145` | 80.7% |
| Deterministic ordering: interfaces lexical, top IDs by count desc then id asc | `scan.rs:300-312` | `contract.rs:127,168` | 80.7% |
| Page limit 1..=5000; top-ID limit ≤64 | `lib.rs:219-231,269-277` | `contract.rs:233` (page). Top-ID refusal **untested** | 78.6% |
| Enrichment catalog: only `driver=="robstride"`, refuses duplicate address and empty names | `robstride.rs:30-64` | **untested** (robstride.rs 0.0%) | 0.0% |
| Enrichment labels (comm type, inbound device id, joint) | `scan.rs:451-487` | **untested.** No test builds the crate with `robstride-enrichment` and asserts a label | in scan.rs, not exercised |

## 6. Inputs/outputs

- **Files:** any capture path supplied by the caller, plain or gzip (`scan.rs:63-97`). In practice these are `/opt/marengo/var/log/candump-latest.log`, written by MCP sessions via `candump -t z` (`tools/marengo-pi-mcp/src/tools/motion.ts:141`), and gzip blobs under `var/log/blobs/` (ADR 0011:13).
- **Config:** `motors.yaml` from `config_dir`, via `marengo_config::load_motors_config_from` (`lib.rs:347-352`). Keys read: `motors[].{driver, joint, can_interface, device_id}` (`robstride.rs:37-61`).
- **Robstride:** comm types 0–24 labels via `robstride::comm` (`scan.rs:459-487`).
- **Outputs:** serde `Inspection` (JSON), key=value text, or Rust values for store and gateway. No env vars, CAN I/O, Chappe, or proto, and no HTTP of its own (the gateway owns the routes).

## 7. Prior review reconciliation

| Prior id | Current status | Evidence |
|---|---|---|
| G17 (finite huge timestamps panic) | **Fixed.** Ledger "verified", PR233, merged main `d03830d`. finding-index.md:66 still says **Open** (stale index, see §8) | `scan.rs:212-247`; `lib.rs:134-145`; `input_validation.rs:8,36`; commits `5084ad0`, `085d701`; batch18-candump-input-validation.md |
| Batch18 additions (DLC match, line/expansion bounds, gzip checksum) | **Fixed** | `scan.rs:21-22,126-148,374-398`; `input_limits.rs:8-95` |
| T14 (log CLI not on default PATH) | **Open.** The MCP still calls bare `marengo-log-cli` and only reports "not found on PATH" (`tools/marengo-pi-mcp/src/tools/logs.ts:125-132`); same in `scripts/pi-remote.sh:115`. The installed binary is `/opt/marengo/bin` (tooling.md:180). Ledger: open | consumer-side, not a crate defect |
| G16 (paged log reads do blocking unbounded work on Tokio workers) | **Open, and it applies to candump.** The gateway handlers `session_candump`, `latest_candump`, `*_summary` are `async fn`s that call store methods doing a full streaming `inspect_path` (up to 256 MiB decompressed) inline, with no `spawn_blocking` (`bins/marengo-gateway/src/logs.rs:426-470`; `marengo-store/src/store.rs:555-640`). Every page request rescans from line 1 | gateway.md G16; ledger open |
| gateway.md design note 12 (duplicate HTTP/proto candump schema) | **Open.** Gateway JSON and proto `CandumpFrame/Page/Summary` duplicate the crate's serde model. The "one-release" aliases from 2026-07 remain (`logs.rs:146-176`) | ADR 0001:34-39; `proto/marengo/v1/marengo.proto:439-484` |

## 8. Drift

1. There is no `crates/marengo-candump/codemap.md` (every other core crate has one), and the crate is missing from the crate tables in crates/codemap.md:14-25, crates/AGENTS.md:12-24, AGENTS.md:50-56 and codemap.md:50-60 (grep finds no `marengo-candump`). It is referenced only from ADR 0001/0011 and bins/marengo-log-cli/codemap.md.
2. finding-index.md:66 lists G17 as Open, while implementation-ledger.json marks it `verified` (PR233). batch18 doc lines 3-8 still say "Integration ... pending".
3. `TimestampMode::Delta` doc says "Zero-based capture timestamps emitted by `candump -t z`" (`lib.rs:55`). The CLI flag `--timestamp delta` (`main.rs:168-171`) and ADR 0001:39 read like can-utils `-t d`, which this crate rejects as regression.
4. `FrameEnrichment.device_id` is documented as "direction-aware" (`lib.rs:172`). It is wrong for type-0/17 replies and for host-sent type-24/2/21-shaped frames (lead L3).
5. `lib.rs:31-33` caller table omits the MCP `pi_candump_summary` and `scripts/pi-remote.sh` (via log-cli) and Consul (via gateway). It also omits that the gateway drops `device_id` from enrichment (`logs.rs:138-145`).
6. Summary docs say `approx_hz` is "count divided by whole-capture duration" (`lib.rs:181,201`). With fewer than 2 distinct timestamps, `duration_s` is 0 and Hz is `None`. That is documented, but the MCP agent rule treats `parsed_frames`/Hz as wire truth while error frames are excluded (lead L1).

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|
| Private `MotorAddress` in `robstride.rs:7-11` | duplicate implementation of `robstride::MotorAddress` (`robstride/src/bus.rs:89-107`, which also has `From<&MotorEntry>`) | med | `robstride.rs` only. The lookup would build a `robstride::MotorAddress` |
| Catalog duplicate-address validation (`robstride.rs:55-60`) | duplicate of `SocketCanRouter::open` (`robstride/src/bus.rs:1394-1407`) and marengo-config validation [INFERENCE: not re-checked in marengo-config] | low | robstride.rs; error text |
| `comm_type_label` table (`scan.rs:472-487`) | duplicate. A `CommunicationType` name or `Display` in robstride would serve both | low | scan.rs; robstride `comm.rs` would gain a name fn |
| `InspectRequest::with_top_id_limit` / `MAX_TOP_ID_LIMIT` / `Error::InvalidTopIdLimit` (`lib.rs:251,269-277`; `scan.rs:46-47`) | scaffold with no production consumer (only `contract.rs:178`; metrics/pub-usage.md:51). Store, gateway and CLI all use the default 10 | low (cheap knob; an operator may want it) | lib.rs, scan.rs, contract.rs:168-190 |
| `UnixMicros::new` public constructor (`lib.rs:117-120`) | no external constructor use [grep: only `scan.rs:222`] | low | lib.rs |
| Dead `filter_map(CanId::new(..).ok())` in `finish` (`scan.rs:307-311`) | dead branch: IDs were validated on ingest (`scan.rs:386`), so the error arm is unreachable | low | scan.rs |
| `let _ = (can_id, interface)` in the non-feature arm (`scan.rs:441-443`; metrics/suppressions.md:483) | suppression scaffold | low | scan.rs |
| Gateway alias fields `delta_s`, `total_frames`, `frame_count`, `bytes` (`bins/marengo-gateway/src/logs.rs:146-176`) | superseded: ADR 0001:36 called them "one-release HTTP aliases" in 2026-07 | med (outside this crate; Consul consumers need checking) | gateway logs.rs, Consul `use-candump-data.ts`/`log-api.ts`, proto |

## 10. Phase-B leads

1. **L1 — Kernel error and RTR frames are invisible in candump summaries.** `CanId::new` rejects anything above 0x1FFF_FFFF (`lib.rs:69-74`), so can-utils error frames (`CAN_ERR_FLAG` 0x20000000, e.g. the `20000004#0001000000000000` RX-overflow frame captured on the Pi in batch36, ledger CS04) are silently counted as malformed lines (`scan.rs:385-386`). The test `contract.rs:114` with fixture `malformed.log:8` (`20000000#00`) **pins this behaviour**. RTR lines `123#R` fail hex parsing. The MCP rule uses `pi_candump_summary` as "wire truth" after motion troubleshooting, so bus-error evidence never appears in `parsed_frames` or `top_ids`. Gap.
2. **L2 — Extended versus standard is inferred from the value, not the wire.** `is_extended` is `value > 0x7FF` (`lib.rs:80-82`). A 29-bit extended ID ≤ 0x7FF is printed as 3 hex digits and is **never enriched** (`scan.rs:456-458`). Example: type-0 UID replies `(0<<24)|(device<<8)|0xFE` for device ≤7, such as the ADR 0036:150 probe `000001FE`. can-utils encodes extended IDs as 8 digits, so the information is in the input but is discarded at `scan.rs:385`.
3. **L3 — Enrichment mislabels direction-specific frames.** `enrich_robstride` (`scan.rs:459-463`) uses `inbound_motor_device_id`. For type-0 replies (low byte 0xFE) and type-17 replies (low byte host 0xFD), it reports device 254/253 and the wrong joint. For host-sent type-24 Off commands (host 0xFD in bits 8-15) it reports device 253. These are exactly the frames ADR 0036:176 asks operators to inspect in candump. `robstride::comm.rs:49-51` warns against this use. Untested (robstride.rs 0% coverage; no enrichment test).
4. **L4 — One out-of-order line aborts the whole inspection.** `TimestampRegression` is a hard error (`scan.rs:202-210`). A multi-interface `candump -t z can0 can1` stream or a clock step [INFERENCE: kernel per-socket stamping can interleave] makes the whole summary or page fail, both in the MCP and in archive import (`store.rs:470-480`, during session archive).
5. **L5 — Blocking full rescans on the gateway's Tokio workers.** Every page or summary request re-reads and re-decompresses the entire capture (≤256 MiB) synchronously in an `async` handler (`bins/marengo-gateway/src/logs.rs:426-470` → `marengo-store/src/store.rs:555-640` → `scan.rs:114-158`). There is no `spawn_blocking`, no index (ADR 0011:13), and offset paging is O(file) per request. This extends G16.
6. **L6 — Enrichment silently degrades.** If motors.yaml fails to load, the gateway logs a warning and falls back to plain (`bins/marengo-gateway/src/main.rs:130-145`). Consul then shows no joint labels with no visible error. The catalog is loaded once at gateway start, so Set Limits or motors.yaml edits are not reflected until restart.
7. **L7 — The Pi CLI probably lacks enrichment.** `deploy-pi.sh:193` builds `marengo-log-cli` with `--features socketcan,linux-i2c` only. log-cli's `robstride-enrichment` feature (`bins/marengo-log-cli/Cargo.toml:19`) gates `--enrich` at compile time (`main.rs:192-204`). [INFERENCE] Cargo feature unification with the gateway turns on `marengo-candump/robstride-enrichment` but not log-cli's own cfg, so `--enrich` errors on the Pi.
8. **L8 — Per-frame allocations.** `parsed.interface.clone()` for every frame into `iface_counts` (`scan.rs:254-257`), `split_whitespace().collect()` and `join` per line (`scan.rs:342,369,380`), and `interface.to_string()` per enrichment lookup (`robstride.rs:22-24`). These cost CPU on 256 MiB captures in the gateway request path (L5).
9. **L9 — Double open and TOCTOU.** `inspect_path` opens the file twice and reads its metadata separately (`scan.rs:68-89`). The hot `candump-latest.log` is being appended by the live `candump`, so `source_bytes` (from metadata) and the parsed content can describe different file states. The unterminated last line is counted as a line (`lib.rs:193-194`), so a frame written halfway through is "malformed" (`total_lines` grows, frames do not).
10. **L10 — `gzip` decided by the first two bytes only.** A plain capture can never start with `0x1f 0x8b`, since it starts with `(`, so the check is safe today. The gzip path does not check `source_bytes` against the decompressed size, which is covered separately by the 256 MiB cap (`scan.rs:139-143`). No action needed beyond confirming.
