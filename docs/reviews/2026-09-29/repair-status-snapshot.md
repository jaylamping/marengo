# Repair status snapshot — October 2, 2026

Generated from the unchanged [implementation ledger](implementation-ledger.json).
**102 findings: 26 verified, 13 partial, 63 open; eight maintenance tasks.**
Verified dispositions describe software scope; physical acceptance stays separate.
Detailed acceptance plans, overlaps and evidence remain in the ledger.

## Verified findings (26)

| ID | Priority | Package | Finding |
|---|---|---|---|
| CS01 | P1 | WP01 | Empty CAN drains renew the global watchdog; one replying motor masks silent peers. |
| CS02 | P1 | WP02 | Torque slew can raise filtered output above the hard cap. |
| CS03 | P1 | WP01 | NaN commands pass checks and encode the extreme negative wire endpoint. |
| CS08 | P1 | WP03 | Disable reports success when every motor write fails. |
| CS09 | P1 | WP03 | Pi shutdown can wait five seconds for disk persistence before stopping motors. |
| CS11 | P2 | WP02 | Initial output bypasses slew; disable leaves a stale limiter value. |
| CS12 | P2 | WP01 | Post-send drain safety errors are discarded. |
| CS23 | P1 | WP16 | Gravity calculation drops joint-origin translations from link COM positions. |
| CS24 | P1 | WP16 | A positive filtered-velocity tail resets the ascent stall fuse after encoder motion stops. |
| G01 | P1 | WP07 | Offline or non-reading IPC peers allow unbounded Pi telemetry queue growth. |
| G02 | P1 | WP09 | Tracing quota never refills after 40 routine events. |
| G03 | P1 | WP09 | Retention re-enters the same Store mutex and deadlocks on an old session. |
| G07 | P1 | WP10 | Legacy motion/zero routes bypass configured token; SPA build tokens are public if set. |
| G13 | P2 | WP09 | Importing one artifact overwrites sibling references with NULL. |
| G14 | P2 | WP09 | Literal-Z parsing falls back to import time for historical sessions. |
| G17 | P2 | WP09 | Huge finite candump timestamps panic during Duration conversion. |
| G18 | P2 | WP08 | Per-core CPU label shifts counters; aggregate iowait reads IRQ. |
| G19 | P2 | WP08 | CAN state parses restart-ms; disk readonly inferred from df without mount flags. |
| F21 | P2 | WP13 | Development preview route breaks four index-based route tests. |
| T16 | P2 | WP12 | Repository tooling directs native Windows work into WSL |
| T17 | P2 | WP15 | Six research search handlers fail on a cache miss |
| T18 | P2 | WP15 | Locked arxiv 4.0 removed the API the code calls |
| T19 | P2 | WP15 | Explicit zero scrape request still performs scraping |
| T20 | P2 | WP15 | Week/month research filters use calendar year |
| T22 | P2 | WP00 | Daily Rust audit uses an unsupported flag |
| T32 | P2 | WP13 | Rust lock has two vulnerabilities and three warnings at baseline |

## Partial findings (13)

| ID | Priority | Package | Finding |
|---|---|---|---|
| CS04 | P1 | WP01 | Status faults are discarded, detailed flags truncated and later status erases faults. |
| CS05 | P1 | WP04 | A same-name persisted row marks reference Verified without current hardware proof. |
| CS06 | P1 | WP04 | Set Zero verifies cached feedback predating the command. |
| CS07 | P1 | WP03 | One-shot set-zero enables all drives and exits without guaranteed cleanup. |
| CS13 | P2 | WP01 | Fault state clears on the next good disabled tick and can be missed or auto-rearmed. |
| CS14 | P2 | WP02 | Danger-zone fault is ignored; velocity-only clipping cannot brake zero-kd gravity mode. |
| CS15 | P2 | WP01 | Invalid numeric policy and duplicate joint mappings pass startup validation. |
| CS21 | P2 | WP16 | Ignored independent dynamics tests are stale and fail when executed. |
| G15 | P2 | WP09 | Interrupted schema migration leaves version behind applied non-idempotent DDL. |
| T01 | P1 | WP10 | Runtime can replace root sudo helpers through writable parents |
| T04 | P1 | WP10 | Loopback Set Limits writer accepts unauthenticated foreign-origin writes |
| T26 | P2 | WP00 | Primary CI omits several behavior suites/dependency audits |
| T31 | P2 | WP13 | Node locks include current advisories; tools lack audit gate |

## Open findings (63)

| ID | Priority | Package | Finding |
|---|---|---|---|
| CS10 | P1 | WP02 | Bench caps bound feedforward only; accepted PD terms can greatly exceed that cap. |
| CS16 | P2 | WP02 | Set Limits checks derivative scratch state, missing the current disabled pose. |
| CS17 | P2 | WP08 | IMU silently republishes old orientation with fresh timestamps. |
| CS18 | P2 | WP08 | A 14-byte rotation report is split as 12 bytes and drops subsequent reports. |
| CS19 | P2 | WP05 | First Testing hold loses requested gains; wave path skips them. |
| CS20 | P2 | WP05 | Several one-shot CLI commands alter only an exiting local process or no-op gains. |
| CS22 | P2 | WP12 | Unconditional Unix IPC prevents native Windows core/workspace compilation. |
| G04 | P1 | WP06 | Latest-request coalescing drops motor/URDF writes or prior callers ACKs. |
| G05 | P1 | WP06 | Global persistence flags allow unsafe restart after failure or unrelated Durable. |
| G06 | P1 | WP06 | Missing/stale Active telemetry permits restart/update/URDF activation. |
| G08 | P1 | WP06 | Disk-based no-op says Durable while divergent live limits stay unchanged. |
| G09 | P1/P2 | WP06 | Gateway activation and Pi persist/rollback race over the same master URDF. |
| G10 | P2 | WP07 | Cached snapshots/health look current after Pi disconnection. |
| G11 | P2 | WP12 | Native Windows Chappe compile fails before gateway/core tests can run. |
| G12 | P2 | WP13 | 13-day TLS certificates are loaded once without live renewal. |
| G16 | P2 | WP09 | Log paging/downloads do blocking unbounded work on Tokio workers. |
| G20 | P2 | WP07 | Failed required HTTP/HTTPS listeners leave gateway nominally running. |
| G21 | P2 | WP06 | URDF activation can return HTTP500 after master promotion and staging removal. |
| F01 | P1 | WP05 | Disable leaves playback timer able to send a later command and re-enable. |
| F02 | P1 | WP05 | Dry Run to live switch during playback bypasses Wave sign-off gate. |
| F03 | P1 | WP05 | Manual Hold Stop changes only a local browser flag. |
| F04 | P1 | WP05 | PID update sends an Impedance motion batch with a stale manual target. |
| F05 | P1 | WP07 | Open gateway stream is treated as fresh robot state indefinitely. |
| F06 | P1 | WP07 | Static inventory fabricates nominal safety/power and Enabled drive states. |
| F07 | P1 | WP03 | Software E-STOP needs two clicks and hides POST failure. |
| F08 | P2 | WP07 | Detached stream reader rejection skips disconnect/reconnect. |
| F09 | P2 | WP07 | HTTP stream cleanup does not abort fetch or cancel reader. |
| F10 | P2 | WP07 | StrictMode asynchronous setup leaks the first telemetry subscription. |
| F11 | P2 | WP05 | Native Wave Loop renews UI countdown without a second finite wave command. |
| F12 | P2 | WP05 | Dry runs wait on real measured settling and never complete. |
| F13 | P2 | WP05 | Teach extractor misses current elbow Wave extrema. |
| F14 | P2 | WP04 | Real Set Zero leaves taught coordinates considered valid. |
| F15 | P2 | WP07 | Telemetry rebuilds Hardware WebGL scene and loses selection. |
| F16 | P2 | WP06 | Optimistic Range override masks newer live limit snapshots forever. |
| F17 | P2 | WP07 | 12/s predecode quota drops WARN/ERROR before severity is known. |
| F18 | P2 | WP09 | Archive responses race to populate the wrong session/page. |
| F19 | P2 | WP09 | Archive/search silently shows only first500/200 rows and filters locally. |
| F20 | P2 | WP07 | Self-update can succeed based on another job installed target. |
| F22 | P2 | WP06 | Optional local sync can hang/fail a Pi save already confirmed Durable. |
| F23 | P2 | WP04 | Set Zero shows Applied on queue publication without verified runtime ACK. |
| F24 | P3 | WP09 | Dormant chart filters reject actual timestamp format and lack requested history. |
| F25 | P3 | WP09 | URDF archive listing fetch occurs during render. |
| T02 | P1 | WP06 | Install overwrites taught limits and continues after preservation failure/no-op |
| T03 | P1 | WP06 | Config/URDF sync bypasses persistence and safe activation |
| T05 | P1 | WP03 | MCP manual sessions do not stop the systemd CAN owner |
| T06 | P1 | WP10 | Shell interpolation permits injected commands, including read-only tools |
| T07 | P2 | WP11 | SSH exit status disappears; failed motion is audited as exit 0 |
| T08 | P2 | WP11 | Schema default defeats script timeout calculation |
| T09 | P2 | WP11 | Negative Wave bounds defeat automatic wait insertion |
| T10 | P2 | WP11 | Git/build tools execute inside deployment directory |
| T11 | P2 | WP11 | Cross deploy switches the operator's working branch |
| T12 | P1 | WP03 | Stop helper returns success without confirming the process stopped |
| T13 | P2 | WP11 | Log-list tool generates invalid Bash |
| T14 | P2 | WP11 | Log tools cannot find installed log CLI under default PATH |
| T15 | P2 | WP12 | macOS deployment assumes newer Bash and GNU checksum tools |
| T21 | P2 | WP00 | Daily audit labels legitimate codegen and test helpers critical |
| T23 | P2 | WP14 | Auto Learn validator accepts incomplete joint limits/live pose |
| T24 | P2 | WP14 | Empty prior bypasses Crawl-first |
| T25 | P2 | WP14 | Auto Learn schedule ignores per-joint velocity caps |
| T27 | P2 | WP16 | Simulation green status exercises only a minimal unactuated model |
| T28 | P2 | WP16 | Production MJCF inertials diverge from production URDF |
| T29 | P3 | WP16 | Export/conversion commands report success without exporting/converting |
| T30 | P2 | WP16 | Partial gravity-preview arguments silently become all zeros |

## Maintenance tasks

| ID | State | Task |
|---|---|---|
| M01 | partial | Replace or isolate unmaintained paste/rustls-pemfile dependencies |
| M02 | open | Drive-local timeout and independent torque-limit handshake |
| M03 | open | Hardware E-stop and Hall method/polarity honesty |
| M04 | open | Wrong-sign policy coordinate and per-pose contract |
| M05 | open | Reject unsupported or malformed dynamics models |
| M06 | partial | Bound realtime command work and measure loop jitter |
| M07 | open | Current CAD and model provenance plus scope honesty |
| M08 | partial | Docker Desktop unexpected disappearance and stranded Windows runtime sockets |
