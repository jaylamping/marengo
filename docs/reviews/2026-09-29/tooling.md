# Marengo tooling, deployment, dependency, CAD and simulation review

Reviewed on **September 29, 2026** against main commit **4bc77ba605834fdec04b436daa4bec67bca84fbb**. Source inspection used the preserved `review-main` checkout. Reproductions were offline handler calls, temporary files, loopback HTTP with a no-op executable, or disposable Linux containers. No SSH to the robot, CAN access, deployment, enable, zeroing, or motion was performed.

This appendix contains **32 findings: 7 P1, 24 P2, and 1 P3**. P1 means correct before the affected deployment/control path is used again; P2 means an actionable functional, validation, portability, or dependency defect; P3 means an explicitly documented scaffold whose completion/status reporting needs improvement. Static proofs are identified separately from reproductions. These priorities describe Marengo impact, independently of advisory-database severity.

Most product defects remain open. The root review task has corrected Windows/WSL guidance and launch configuration, added Consul tests to the primary gate, and refreshed Consul/Rust locks. Those small corrections are identified below. They do not resolve the motion, permission, or persistence defects.

## Finding and fix index

| ID | Priority | Finding | Recommended fix |
|---|---|---|---|
| [T01](#t01--root-owned-helper-files-are-replaceable-through-runtime-writable-directories) | P1 | Runtime can replace root sudo helpers through writable parents | Root-own immutable code and every ancestor; separate writable state |
| [T02](#t02--taught-limit-preservation-fails-open-and-some-failed-patches-report-restoration) | P1 | Install overwrites taught limits and continues after preservation failure/no-op | Stage, verify, atomically promote or roll back; retain backups on failure |
| [T03](#t03--config-and-urdf-sync-can-overwrite-the-pis-durable-taught-state-without-activation-checks) | P1 | Config/URDF sync bypasses persistence and safe activation | Route through one Pi transaction with Disabled/freshness and generation checks |
| [T04](#t04--the-loopback-limit-writer-processes-foreign-origin-requests-without-authentication) | P1 | Loopback Set Limits writer accepts unauthenticated foreign-origin writes | Reject origins, authenticate, require JSON, bound body/worker execution |
| [T05](#t05--manual-mcp-sessions-can-compete-with-the-systemd-motor-owner) | P1 | MCP manual sessions do not stop the systemd CAN owner | Stop through authorized helper; verify exclusive ownership; prefer IPC commands |
| [T06](#t06--generated-ssh-shell-scripts-allow-command-injection) | P1 | Shell interpolation permits injected commands, including read-only tools | Use argument vectors/single-quote literals; allowlist protocol inputs |
| [T07](#t07--failure-status-is-lost-between-ssh-tool-handlers-and-mcp) | P2 | SSH exit status disappears; failed motion is audited as exit 0 | Preserve structured results through handlers and MCP responses |
| [T08](#t08--the-schemas-5-second-timeout-default-defeats-script-duration-inference) | P2 | Schema default defeats script timeout calculation | Make timeout optional; calculate/validate complete execution budget |
| [T09](#t09--negative-wave-bounds-are-not-recognized-by-automatic-wait-insertion) | P2 | Negative Wave bounds defeat automatic wait insertion | Parse commands structurally and calculate waits from parsed numbers |
| [T10](#t10--native-buildgit-tools-use-the-runtime-directory-instead-of-the-source-clone) | P2 | Git/build tools execute inside deployment directory | Separate source/staging/runtime paths in types and operations |
| [T11](#t11--cross-deployment-changes-the-operators-active-checkout) | P2 | Cross deploy switches the operator's working branch | Build an immutable SHA in isolated staging/worktree; preserve active checkout |
| [T12](#t12--the-stop-helper-does-not-prove-its-requested-stop-happened) | P1 | Stop helper returns success without confirming the process stopped | Fail if service/process remains; expose explicit stop outcome |
| [T13](#t13--the-log-list-tool-emits-a-syntax-error-before-listing-files) | P2 | Log-list tool generates invalid Bash | Keep pipeline on one logical command; execute syntax checks in tests |
| [T14](#t14--candumparchive-tooling-expects-a-log-cli-that-default-install-path-does-not-expose) | P2 | Log tools cannot find installed log CLI under default PATH | Use an absolute installed binary path |
| [T15](#t15--native-mac-deploy-has-undeclared-shelltool-prerequisites) | P2 | macOS deployment assumes newer Bash and GNU checksum tools | Portable checksum/array implementation or explicit tested prerequisites |
| [T16](#t16--baseline-mcp-launchers-and-hooks-enforce-an-obsolete-wsl-workspace-preference) | P2 | Repository tooling directs native Windows work into WSL | Native Node launchers and host-aware instructions; corrected by root task |
| [T17](#t17--six-cached-search-handlers-pass-unawaited-coroutines-into-response-validation) | P2 | Six research search handlers fail on a cache miss | Await returned awaitables; test public handlers with empty cache |
| [T18](#t18--locked-arxiv-version-removed-searchresults) | P2 | Locked arxiv 4.0 removed the API the code calls | Use Client.results and keep a lock-version compatibility test |
| [T19](#t19--an-explicit-scrape_top_n0-is-treated-as-the-configured-nonzero-default) | P2 | Explicit zero scrape request still performs scraping | Distinguish absent values from zero |
| [T20](#t20--research-weekmonth-recency-is-only-a-year-filter) | P2 | Week/month research filters use calendar year | Store publication timestamps and apply actual time windows |
| [T21](#t21--deterministic-daily-audit-reports-false-critical-findings-for-valid-changes) | P2 | Daily audit labels legitimate codegen and test helpers critical | Regenerate/compare codegen and inspect production code/diffs |
| [T22](#t22--optional-daily-cargo-audit-fails-before-scanning) | P2 | Daily Rust audit uses an unsupported flag | Use --no-fetch and distinguish scanner errors from findings |
| [T23](#t23--auto-learn-safety-validation-accepts-incomplete-joint-context) | P2 | Auto Learn validator accepts incomplete joint limits/live pose | Require complete, unique, valid context for every commanded joint |
| [T24](#t24--empty-prior-landmarks-bypass-crawl-first) | P2 | Empty prior bypasses Crawl-first | Require a validated accepted predecessor and stage progression |
| [T25](#t25--generated-schedule-validation-does-not-use-actual-velocity-caps) | P2 | Auto Learn schedule ignores per-joint velocity caps | Validate trajectory derivatives against real caps |
| [T26](#t26--cis-primary-gate-leaves-production-behavior-suites-and-tool-dependency-audits-outside-the-gate) | P2 | Primary CI omits several behavior suites/dependency audits | Gate all production tools and Python/shell behavior tests |
| [T27](#t27--a-green-simulation-job-does-not-exercise-the-production-control-or-safety-path) | P2 | Simulation green status exercises only a minimal unactuated model | Parse/step production model and run the actual control/safety path |
| [T28](#t28--production-mjcf-does-not-mirror-urdf-inertial-properties) | P2 | Production MJCF inertials diverge from production URDF | Generate/compare masses, COMs, inertias, frames, axes and limits |
| [T29](#t29--cad-export-and-urdf-conversion-commands-are-success-reporting-scaffolds) | P3 | Export/conversion commands report success without exporting/converting | Expose scaffold status; require generated provenance and real conversion |
| [T30](#t30--partial-gravity-preview-vectors-silently-evaluate-the-zero-pose) | P2 | Partial gravity-preview arguments silently become all zeros | Reject incomplete vectors or accept named angles; update MCP/harness |
| [T31](#t31--node-dependency-audits-reveal-production-tool-and-consul-advisories) | P2 | Node locks include current advisories; tools lack audit gate | Patch compatible locks, test SDK changes, plan Router migration |
| [T32](#t32--baseline-rust-lock-contains-patched-vulnerabilities-and-maintenance-warnings) | P2 | Rust lock has two vulnerabilities and three warnings at baseline | Patch h2/rustls/anyhow; migrate unmaintained dependencies |

## Deployment, permissions and control tooling

### T01 — Root-owned helper files are replaceable through runtime-writable directories

**Source:** [scripts/install-pi.sh:121–130,158–166,243–255](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/install-pi.sh#L121).

The installer recursively assigns `/opt/marengo` to the runtime user, then makes `scripts/` group writable. It re-hardens two helper *files* as root:root 0755, but leaves their parent writable by the same user granted passwordless sudo execution of those paths. File permissions do not stop that user replacing a directory entry. Compromise of the gateway/runtime account can therefore replace an allowed helper and execute its replacement as root. The deploy account is separately granted execution of a script in its home checkout; that is deliberately a fully trusted deployment principal, and should be documented as such.

**Verified:** `tooling-permission-repro.py` ran in a disposable Linux container. UID 1000 successfully replaced a root-owned 0755 helper in a root:1000 0775 directory. The replacement was not executed and no real sudo target was touched. This is a permission proof of the install layout, not evidence that the current Pi has been compromised.

**Fix:** put privileged helpers under a root-owned 0755 directory such as `/usr/local/libexec/marengo`, with root-owned non-writable ancestors. Keep release binaries/scripts immutable to the runtime user, with write access only to configuration/state locations explicitly needed. Validate arguments inside privileged helpers, narrow sudo rules, and treat the deployment account as administrator-equivalent. Add a disposable install-layout test that tries rename/replacement as the runtime UID and expects denial.

### T02 — Taught-limit preservation fails open, and some failed patches report restoration

**Source:** [scripts/install-pi.sh:79–105,257–272](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/install-pi.sh#L79); [scripts/preserve-taught-limits.py:89–95,130–170,220–244,250–293](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/preserve-taught-limits.py#L89).

Install first replaces configuration with rsync. Backup copies suppress errors; a missing preservation script or Python failure only prints a warning. The backup is then deleted, deployment revision is stamped, and services restart. A fresh image lacking PyYAML is one concrete failure path. Independently, the patch functions return unchanged text when the destination joint has no numeric hard/soft keys; the caller records it as restored and returns success. A legitimate change to YAML shape can silently discard previously taught envelopes.

**Verified:** the offline Python reproduction supplies an installed motor with only a torque cap and an installed control entry with only a velocity cap. Both hard/soft patch functions return the original text unchanged. The unconditional continuation/deletion/restart behavior is a direct source proof.

**Fix:** merge into a staging tree before altering the live release. Require complete backups and dependencies; parse the final staged files and compare every preserved joint's hard/soft/URDF values to the expected result. Insert absent fields correctly or reject unsupported layouts. Abort and retain the backup on any missing/failed preservation, without stamping success or restarting into changed limits. Promote the validated release/configuration atomically; keep a recoverable old generation. Test absent keys, alternate numeric YAML forms, missing PyYAML, malformed files, backup failures and partial writes.

### T03 — Config and URDF sync can overwrite the Pi's durable taught state without activation checks

**Source:** [tools/marengo-pi-mcp/src/tools/sync-config.ts:47–48,75–105,168–196,270–310](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/sync-config.ts#L47); [src/tools/admin.ts:90–114](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/admin.ts#L90).

The default `install_to_opt:true` path directly rsyncs all master YAML files into live configuration. URDF sync directly replaces the installed file. Neither path obtains a fresh Disabled state, checks pending/degraded persistence, performs preservation, nor compares configuration generations. The URDF tool prints an ADR warning, but a Durable status only confirms the Pi write succeeded; it does not make an older local URDF safe to push. Subsequent process start can therefore boot stale hard/soft limits or a mismatched model. Writes can also race the Pi's asynchronous persistence worker.

**Verified:** source inspection of the generated install bodies and their defaults. No live sync was attempted. This overlaps the gateway/configuration review's single-writer and activation findings; use one shared repair rather than two separate sync implementations.

**Fix:** make Pi configuration authority the only writer. Send a versioned staged update with an expected generation, preserve taught envelopes by default, validate hardware/model identities, and require a fresh safe activation state. Return a durable acknowledgment covering every changed file and whether restart is required. Provide an explicit separately confirmed replacement mode for intentional recalibration, instead of unconditional whole-tree overwrite. Test stale generation, pending writes, Enabled/unknown state, concurrent URDF persistence and partial promotion.

### T04 — The loopback limit writer processes foreign-origin requests without authentication

**Source:** [tools/limit-sync-local/server.ts:20–26,37–60,84–112](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/limit-sync-local/server.ts#L20).

The origin allowlist controls only the response CORS header. A disallowed Origin is still allowed to POST and execute the local config-writing binary. The endpoint accepts JSON carried as `text/plain`, so a simple browser POST can cause a state change even though the attacking page cannot read the response. It also buffers unlimited body bytes and invokes a synchronous child without an execution timeout. Browser private-network restrictions vary; the server-side authorization defect was reproduced independently of any browser policy.

**Verified:** `tooling-limit-cors-repro.mjs` ran only on loopback with `tooling-noop.exe` replacing the real writer. A POST from `https://untrusted.example`, content type `text/plain`, returned 200 `synced`, with no allow-origin response header. No real config was changed.

**Fix:** reject disallowed/missing browser origins according to a deliberate policy, require a per-session authorization token and JSON content type, and keep loopback binding. Bound request bytes, time, queue and child execution. Use asynchronous execution and return validated writer errors. Confirm foreign-origin, wrong token/content type, oversize body and stalled worker are rejected before invoking the writer.

### T05 — Manual MCP sessions can compete with the systemd motor owner

**Source:** [tools/marengo-pi-mcp/src/tools/motion.ts:154–179,327–348,373–390,701–729](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/motion.ts#L154); [scripts/systemd/marengo-pi.service:9–16](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/systemd/marengo-pi.service#L9); [scripts/install-pi.sh:270–272](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/install-pi.sh#L270).

Motion sessions attempt a plain `pkill` as the SSH deploy user (default `joey`) and suppress failures, then run another `marengo-pi` or `motor-repl`. The installed service runs as `marengo`, and the deploy user's group membership does not grant permission to signal that different UID. Even a successful process kill would not stop an active `Restart=always` unit. The first instance can continue commanding CAN while the second disables/enables/zeros the same drives. The installer actually enables/restarts the service despite its closing instruction suggesting manual ownership.

**Verified:** source proof of default users, suppressed pkill failure, service restart policy and subsequent process launch. No live owner or process state was queried. This is a tooling counterpart of the control review's multiple-owner problem.

**Fix:** prefer sending commands to the single long-lived Pi owner over IPC. Until then, call the authorized stop helper in stop mode, require a confirmed stopped state, and acquire an OS-level exclusive lease before any process opens the configured CAN/motor set. Refuse a second owner, and release through guaranteed cleanup. Test service UID mismatch, automatic restart, a second process, stale lease, interrupted SSH and cleanup failures in disposable/stub environments.

### T06 — Generated SSH shell scripts allow command injection

**Source:** [tools/marengo-pi-mcp/src/tools/motion.ts:292–324,435–451,598–618](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/motion.ts#L292); [src/harness/index.ts:99–109](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/harness/index.ts#L99); [src/tools/readonly.ts:97–112,164–179](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/readonly.ts#L97); [src/paths.ts:5–24](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/paths.ts#L5); [src/config.ts:67–83](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/config.ts#L67).

`JSON.stringify` makes a JavaScript string literal, not a shell literal: its double quotes preserve `$()` and backtick substitution. Raw joint/operator interpolation also permits separators. This affects both motion script lines and tools labelled read-only: an allowlisted log-path prefix followed by `$(...)` passes the path check and is interpolated into `cat`. A caller that authorizes a robot command or file read has not thereby authorized arbitrary shell execution. The default deploy account's installation sudo privilege increases the consequences.

**Verified:** captured script `status $(printf SUBSTITUTED)` becomes a printf line whose harmless local Bash execution prints `status SUBSTITUTED`; the supplied text is substituted. `pi_jog` captures a second `printf` command after an injected semicolon. `pi_read_file` accepts `/opt/marengo/var/log/$(printf READ_INJECTED)` and generates an expandable double-quoted cat path. No remote command or harmful payload was executed.

**Fix:** use the existing `shellQuote` helper for every literal passed into Bash, or a constrained remote runner carrying structured argument vectors rather than arbitrary source. Validate joint names against the commissioned map, operator identifiers against a narrow grammar, finite numeric values, and supported protocol command syntax. Normalize and use the validated path, with symlink-aware read restrictions where needed. Add tests for dollar substitution, backticks, quotes, semicolons, newlines and leading-option filenames across all tool families.

### T07 — Failure status is lost between SSH, tool handlers and MCP

**Source:** [tools/marengo-pi-mcp/src/ssh.ts:14–18,70–84,138–143](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/ssh.ts#L14); [src/index.ts:29–32,78–88](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/index.ts#L29); [src/tools/motion.ts:484–485,616–618,795–797,841–855](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/motion.ts#L484).

The SSH layer returns an exit code, but `runRemote` immediately turns it into text. Most motion handlers then append an audit record with literal exitCode 0. The MCP error heuristic does not recognize `[exit 1]` or `[exit 124]`. A failed stop, enable or timeout can therefore be returned as a successful MCP call and a successful audit record. Some deploy paths separately parse the marker, demonstrating the inconsistency.

**Verified:** the offline reproduction returns `[exit 1]` through an actual read-only handler; the exact index heuristic classifies it `isError:false`. Static call-site inspection confirms motion audits always receive 0.

**Fix:** pass `RemoteExecResult` through each handler. Set MCP `isError` from structured non-zero status, preserve stdout/stderr as evidence, and audit the actual status on both successful and failed attempts. Make blocked motion a typed error distinct from process failure. Test exit 1, timeout 124, spawn failure, empty output, and a success message printed before a failed exit.

### T08 — The schema's 5-second timeout default defeats script duration inference

**Source:** [tools/marengo-pi-mcp/src/tools/motion.ts:238–242,748–781](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/motion.ts#L238).

The handler infers `sleepBudget + 10` only when `timeout_sec` is absent. Zod inserts default 5 before the handler, so the inference branch is unreachable for ordinary MCP requests. A script with `sleep 35` and no explicit timeout is sent to `timeout 5`, interrupting the intended motion/return/disable sequence.

**Verified:** the real input schema parses an omitted timeout as 5; the captured generated pipe contains `timeout 5` for a 35-second dwell. No robot script ran.

**Fix:** make the field optional and calculate the default after parsing. Reject explicit timeouts shorter than required waits, return-home time and shutdown allowance; derive SSH timeout from that complete budget. Test the public schema-to-handler path, including long dwells and scripts at the configured maximum duration.

### T09 — Negative Wave bounds are not recognized by automatic wait insertion

**Source:** [tools/marengo-pi-mcp/src/tools/motion.ts:256–270](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/motion.ts#L256).

The Wave regex accepts only digits and dots for position bounds. Valid negative limits, such as the current elbow's negative lower range, fail matching, so the generated script proceeds immediately to its next command/quit without waiting for the requested cycles. Exponents are another unsupported numeric spelling.

**Verified:** `expandScriptWithWaveWaits(['wave right_elbow_pitch -0.5 1.2 3 0.4'])` returns only the original command and no sleep.

**Fix:** parse Wave using the same command grammar as the runtime, validate numeric domains, and calculate duration from cycles/half-period. Avoid duplicating an ad hoc regex grammar across tooling. Test negative/exponent bounds, omitted period, invalid cycles and a following disable.

### T10 — Native-build/Git tools use the runtime directory instead of the source clone

**Source:** [tools/marengo-pi-mcp/src/config.ts:38–59](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/config.ts#L38); [src/env.ts:14–18](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/env.ts#L14); [src/tools/admin.ts:154–199](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/admin.ts#L154); [src/tools/clean-tree.ts:19–41](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/clean-tree.ts#L19); [src/tools/sync-tree.ts:7–28](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/sync-tree.ts#L7); [src/tools/deploy.ts:42–53](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/deploy.ts#L42).

The shared preamble always changes directory to default `/opt/marengo`. Installer output there contains bins/config/assets/scripts/www, without `.git`, `Cargo.toml`, crates or Consul source. Git pull/sync/clean, `pi_build`, and the initial `pi_native` resolution therefore fail in the standard installed layout. The config already has `piStagingRoot` (default `~/marengo`) for the actual source/staging clone, but these operations do not use it.

**Verified:** source inspection of default path selection, generated commands and installer layout. Existing unit tests assert command fragments but do not model the installed directory contents.

**Fix:** model `RuntimeRoot`, `SourceRoot` and immutable `BuildRoot` separately, with helpers that operate in the correct one. Resolve native-update Git from staging/source, keep health/motion on runtime paths, and reject an invalid source root before stopping services. Test a realistic installed tree with no Git metadata plus a separate clone.

### T11 — Cross deployment changes the operator's active checkout

**Source:** [tools/marengo-pi-mcp/src/tools/deploy.ts:103–147](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/deploy.ts#L103).

The cross strategy fetches, checks out main and pulls inside `cfg.localRoot`, and leaves that branch selected. Its name deliberately targets main, but a deployment operation still mutates the shared development workspace and cannot deploy a reviewed feature-branch SHA. It also only checks tracked dirtiness; untracked working files are excluded. This is a workflow/architecture concern rather than a claim that the tool's documented main target is accidental.

**Fix:** accept an explicit reviewed target SHA, or resolve main once and build in isolated staging/worktree without changing the caller's branch. Stamp that exact SHA with config/model hashes and verify the resulting installed manifest. A retained orphan branch contains a useful current-HEAD deployment change; do not copy its whole old deploy file because that would revert the newer Pi-native update path. Test deploy selection from a feature branch and assert the original checkout remains unchanged.

### T12 — The stop helper does not prove its requested stop happened

**Source:** [scripts/pi-restart-marengo-pi.sh:20–27,44–52](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/pi-restart-marengo-pi.sh#L20); `tools/marengo-pi-mcp/src/tools/restart-marengo-pi.ts`.

Stop mode suppresses systemctl/pkill errors, waits a bounded number of polls, then prints the final status and exits successfully even if the service/process remains. The poll reaching its final iteration is not an assertion. Together with T07, a caller can receive successful tool status for an unconfirmed stop. Process patterns also primarily cover the installed path, so staging binaries need explicit ownership handling.

**Verified:** static control-flow proof; no actual stop was requested. The existing helper test exercises the happy path, not permission denial or a process that remains alive.

**Fix:** check service inactive and the exact owned process/lease absent after the deadline; return a non-zero typed failure if either remains or status is unknown. Track the owner PID/systemd unit rather than broad string matching. Do not start manual control on an unconfirmed outcome. Test stop denial, persistent process, alternate executable path and restart activation failure with command stubs.

### T13 — The log-list tool emits a syntax error before listing files

**Source:** [tools/marengo-pi-mcp/src/tools/logs.ts:68–73](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/logs.ts#L68).

The generated script has a complete `ls ...` command followed by a new line starting `| head ...`. Bash rejects a pipeline operator at the start of a new complete command, so `pi_logs_list` fails even when files/gateway exist.

**Verified:** actual registered handler captured by `tooling-more-repro.mjs`; `bash -n` reports line 18 `syntax error near unexpected token |`. No SSH executed.

**Fix:** build the pipeline on one logical line or end the previous line with the pipe/continuation. Run `bash -n` on every generated script, and test this read-only listing against temporary log fixtures rather than only substring assertions.

### T14 — Candump/archive tooling expects a log CLI that default install PATH does not expose

**Source:** [tools/marengo-pi-mcp/src/tools/logs.ts:105,128–132](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/logs.ts#L105); [src/env.ts:11–17](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/env.ts#L11); [scripts/install-pi.sh:65–66](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/install-pi.sh#L65); `scripts/env.example`.

The installer places `marengo-log-cli` in `/opt/marengo/bin`. Remote PATH adds Cargo and `/usr/local/cargo/bin`, but not the installed bin directory, and the default environment file does not add it. Candump summary therefore reports the CLI missing under the standard layout; archive fallback silently suppresses the same failure. Being in `/opt/marengo` does not put its `bin` child on PATH.

**Fix:** resolve the executable as an absolute validated `${piRoot}/bin/marengo-log-cli` path, with a deliberate source-build fallback if required. Surface unavailable CLI as a structured error. Test standard PATH with only the installed binary present.

### T15 — Native Mac deploy has undeclared shell/tool prerequisites

**Source:** [scripts/deploy-pi.sh:67–68,95–98](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/deploy-pi.sh#L67); [scripts/deploy-lib.sh:353–371](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/deploy-lib.sh#L353); [scripts/setup-mac-pi-cross.sh:12–31](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/setup-mac-pi-cross.sh#L12).

Native deploy calls GNU `sha256sum` and Bash namerefs (`local -n`). macOS's bundled Bash 3.2 lacks namerefs (requires Bash 4.3+), and its standard checksum tool is `shasum`. The setup script installs the cross compiler/Rust target, but does not install/configure those two prerequisites. Thus a MacBook can successfully set up Rust and still fail before staging/SSH. The Docker build strategy is useful, but does not by itself make every host-shell wrapper portable.

**Verified:** static portability inspection, not a Mac runtime test.

**Fix:** prefer a portable checksum helper (`sha256sum` or `shasum -a 256`) and return arguments without namerefs, or require/test Homebrew Bash 4.3+ and coreutils with an explicit executable choice. Add a macOS CI smoke for argument/host/SSH preparation with no remote operations. The root guide documents the remaining prerequisite; implementation portability remains open.

### T16 — Baseline MCP launchers and hooks enforce an obsolete WSL workspace preference

**Source:** [.cursor/mcp.json:14–20](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/.cursor/mcp.json#L14); [.cursor/hooks/session-start-marengo.ts:109–119](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/.cursor/hooks/session-start-marengo.ts#L109); [.cursor/hooks/check-powershell-shell.ts:114](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/.cursor/hooks/check-powershell-shell.ts#L114); `cad/README.md:26–28,93`.

The baseline loads both MCPs through `bash` and repeatedly tells native Windows sessions to move software into WSL. On this host the `bash` command selects the WSL launcher, defeating the user's requested native `J:\code` workspace. Native Node launchers already exist, so this is configuration/instruction drift rather than a fundamental platform requirement. Windows native Rust IPC compilation is a separate control/gateway finding.

**Fix/status:** root task changed MCP launch to `node .../dist/launch.js`, updated hooks and generated JS, and replaced the WSL-primary docs with Windows/Mac host development and container checks. Keep SolidWorks/CAD on Windows under `J:\code`; use the same tracked source on the Mac. Validate launcher discovery/build on both hosts and avoid OS-specific absolute paths in tracked config.

## Research, automated audits and generated plans

### T17 — Six cached search handlers pass unawaited coroutines into response validation

**Source:** [tools/marengo-research-mcp/src/marengo_research_mcp/tools/search.py:24–46,78–143](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-research-mcp/src/marengo_research_mcp/tools/search.py#L24).

Each async provider is wrapped in a synchronous lambda. `_cached_search` checks whether the *lambda function* is a coroutine function, receives false, and stores the returned coroutine as `hits`. Pydantic response construction then fails outside the exception block; the source never runs. GitHub, Reddit, web, forums, vendor-doc and Hugging Face tool handlers are affected on a cache miss.

**Verified:** mocked async provider plus empty temporary cache causes `ValidationError` without network access. Existing six research tests pass because they test helpers/providers rather than these public handler wrappers.

**Fix:** call the function, then await its result when `inspect.isawaitable(result)`, or standardize every provider behind one async adapter. Include response validation in the handled failure path and cache only valid completed results. Test every public search handler on cache miss/hit and provider error.

### T18 — Locked arxiv version removed Search.results

**Source:** [tools/marengo-research-mcp/src/marengo_research_mcp/sources/arxiv.py:12–17](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-research-mcp/src/marengo_research_mcp/sources/arxiv.py#L12); `uv.lock:28–37` (arxiv 4.0.0).

The code calls `search.results()`, which does not exist in the locked arxiv 4.0.0 package. This source always raises before fetching a paper, degrading both papers search and the orchestrator.

**Verified:** direct offline call under the locked environment returns `AttributeError: 'Search' object has no attribute 'results'` without contacting arXiv.

**Fix:** use `arxiv.Client().results(search)` through the provider adapter and pin compatibility with the selected major API. Test iteration against a stub Client and include an import/API smoke using the lock's installed version.

### T19 — An explicit scrape_top_n=0 is treated as the configured nonzero default

**Source:** [tools/marengo-research-mcp/src/marengo_research_mcp/tools/research.py:77–85,100–112](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-research-mcp/src/marengo_research_mcp/tools/research.py#L77).

`scrape_top_n or cfg.max_scrape` treats zero as missing. A caller explicitly requesting search without scraping still performs extra network work; this can change latency, cost and the data touched by an audit.

**Verified:** the offline orchestrator stub receives `scrape_top_n=0` and still invokes its mocked scraper once for one hit.

**Fix:** distinguish `None`/omission from zero, clamp nonnegative counts deliberately, and keep schema/function defaults aligned. Test omitted/zero/positive/negative counts and confirm zero never calls the scraper.

### T20 — Research week/month recency is only a year filter

**Source:** [tools/marengo-research-mcp/src/marengo_research_mcp/tools/research.py:61–69](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-research-mcp/src/marengo_research_mcp/tools/research.py#L61); `models.py:21–33`.

Both week and month select the current calendar year, and unknown-year hits always pass. A January paper can appear in a late-September “week” result; the model stores only year so a real window cannot be applied.

**Fix:** carry optional publication/update timestamps from providers, apply UTC date cutoffs, and clearly label unknown-date results instead of claiming they satisfy recency. Use provider-native temporal filters where supported. Test year boundary, January-vs-September, unknown dates and recent hits.

### T21 — Deterministic daily audit reports false critical findings for valid changes

**Source:** [scripts/daily-audit/audit.py:130–163,201–236](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/daily-audit/audit.py#L130).

Any changed generated TS path is labelled hand-edited, even when the proto and checksum are legitimately regenerated. The CAN-bypass check scans test modules without using its own test-stripping helper, so Berthier's valid test-only Robstride imports become critical safety findings. Both checks inspect file/path presence rather than the violating production behavior/diff.

**Verified:** the offline reproduction passes a proto + regenerated TS + checksum change and receives critical “Generated consul proto file modified.” Scanning the actual baseline Berthier loop yields a critical bypass report from valid test code.

**Fix:** verify codegen by regenerating with pinned tools and comparing output. Strip/parse cfg(test) modules for architecture checks, inspect actual added production dependencies/calls, and retain precise line evidence. Add false-positive fixtures for legitimate regeneration and test-only MemoryBus/Robstride use plus true-positive fixtures for manual generated edits and production CAN bypass.

### T22 — Optional daily cargo audit fails before scanning

**Source:** [scripts/daily-audit/run.sh:24–47](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/daily-audit/run.sh#L24).

The script invokes `cargo audit --disable-fetch`, a cargo-deny spelling, while installed cargo-audit supports `--no-fetch`. The argument is rejected before scanning, then the wrapper appends a dependency-finding warning as if advisories were found. Scanner failure and vulnerability results are conflated.

**Verified:** disposable container invocation returns `unexpected argument '--disable-fetch' found`; help lists `-n, --no-fetch`. No dependency scan occurred in that reproduction.

**Fix:** use the supported no-fetch flag with an explicitly prepared database; distinguish unavailable scanner, stale/missing database, operational error and actual advisory findings. Capture both output streams and exit type. Test the real installed CLI contract and the wrapper's error classifications.

### T23 — Auto Learn safety validation accepts incomplete joint context

**Source:** [tools/compound-auto-learn/src/parse.ts:30–49,59–84](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/compound-auto-learn/src/parse.ts#L30); `shared/asserts.ts:56–60,156–163,209–230,243–263`.

Request parsing does not require each base joint to appear exactly once in the supplied limits/live pose. The assertion loops skip an unknown limit or missing live position. It can require a landmark elbow coordinate while never checking that elbow's bounds or initial transition. The request also permits inverted position intervals and duplicate joint names.

**Verified:** an offline parsed request names shoulder and elbow but supplies only shoulder limits/live pose. A generated elbow path of +1000 to -1000 radians is accepted. This is a plan-validation failure; it is not a claim that the runtime would physically execute those values without its separate clamps.

**Fix:** before generation/Apply, require complete unique commissioned joint context, finite ordered bounds, valid caps, matching joint names, and fresh pose/config generation. Reject unknown landmark keys and missing limits/live pose. Validate all included commanded joints rather than skipping unavailable evidence. Add malformed-context tests and require backend safety validation independently of browser checks.

### T24 — Empty prior landmarks bypass Crawl-first

**Source:** [tools/compound-auto-learn/shared/asserts.ts:62–77](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/compound-auto-learn/shared/asserts.ts#L62); `src/parse.ts:49`.

Only `null` means “no prior.” An empty array passes Walk/Run, so the claimed first-generation Crawl requirement is not enforced. An arbitrary nonempty prior is also accepted without confirming a valid previous stage or accepted physical signoff.

**Verified:** `assertCrawlFirst('run', [])` returns `{ok:true}`.

**Fix:** represent accepted predecessor state explicitly, including preset/config identity, validated landmarks, stage and operator acceptance. Require a nonempty valid predecessor and legal Crawl→Walk→Run progression; reset it when reference/model/limits change. Test empty/incomplete prior, cross-preset prior, stale generation and skipped stages.

### T25 — Generated schedule validation does not use actual velocity caps

**Source:** [tools/compound-auto-learn/shared/asserts.ts:268–326](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/compound-auto-learn/shared/asserts.ts#L268); `src/parse.ts:38–39`.

The request carries per-joint velocity limits, but the schedule assert checks only a stage-wide minimum duration. A trajectory can pass ROM step and stage duration checks while exceeding a commissioned joint's smaller velocity cap. Even average Δq/time is not checked, and actual interpolation may have a higher peak derivative. Torque caps are similarly context rather than a proved simulation envelope; a model-generated plan cannot be treated as a dynamics safety proof.

**Verified:** static call/dataflow proof. The offline reproduction's valid shoulder step is accepted with a 0.01 rad/s cap and schedule average above that cap, independently of the omitted elbow context defect.

**Fix:** materialize the exact planner trajectory and validate peak velocity/acceleration against each supplied cap, including live-to-first transition and dwell handling. Reject or explicitly retime invalid plans. Keep Davout authoritative and evaluate torque/gravity limits through a separate model/simulation check. Test tiny velocity limits, peak-vs-average discrepancy and synchronized multi-joint trajectories.

### T26 — CI's primary gate leaves production behavior suites and tool dependency audits outside the gate

**Source:** [scripts/check.sh:66–109](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/check.sh#L66); [.github/workflows/ci.yml:171–236](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/.github/workflows/ci.yml#L171); `justfile:78–81`.

At baseline the primary gate builds Consul but does not run its Vitest suite; it tests Pi MCP but only typechecks the Python research launcher and local limit writer. Compound Auto Learn tests, Python handler/daily-audit/preservation tests, and shell behavior tests are not invoked. Npm auditing is limited to Consul, leaving tool production dependencies outside the release gate. This explains why T17/T18 and the actual log-script syntax bug survived green build/unit checks.

**Verified:** source inspection plus passing omitted pure suites. Root task has added Consul Vitest to the gate and fixed the pre-existing route-index failures. The remaining suites/audits are still open work.

**Fix:** add explicit independent checks for compound, Python research handlers, taught-limit preservation, daily audit, local writer authorization and generated shell syntax/behavior. Audit each production npm lock at the chosen release severity. Run meaningful platform smoke checks on Windows/Mac in addition to Linux. Keep privileged/hardware tests isolated and never make default CI enable motors.

## CAD/model provenance and simulation

### T27 — A green simulation job does not exercise the production control or safety path

**Source:** [scripts/check-sim.sh:7–18](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/check-sim.sh#L7); `sim/scripts/smoke_test.py:14–22`; [crates/sim-harness/src/lib.rs:35–38,65–75](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/sim-harness/src/lib.rs#L35).

Default simulation loads `sim/fixtures/minimal.xml`, steps 500 times without commands, and checks only at least one DOF. Production Rust validation counts `type="hinge"` strings and compares counts. It does not even parse the production MJCF through MuJoCo, compare joint identities, check finite state, or run Berthier/Davout against a simulated plant. The current production MJCF has `nu=0` actuators. These are useful bootstrap checks, but provide very little evidence for control commissioning.

**Verified:** five sim-harness tests pass. An additional offline production smoke parses and steps successfully with nq=5/nv=5/nu=0; this does not exercise any motor commands.

**Fix:** make production MJCF parse/step part of the gate, check named joints/axes/limits and finite state, and run the actual control/safety path through a deterministic simulated bus/plant. Add independent gravity, hold, watchdog, stale-feedback, stop/cancel and limit regressions with clear tolerances. Keep model smoke and control safety evidence as distinct statuses.

### T28 — Production MJCF does not mirror URDF inertial properties

**Source:** `assets/mjcf/marengo.xml:2,7–22`; `assets/urdf/marengo.urdf:23–40,53–56,69–72`.

The MJCF claims to mirror the right 5-DOF URDF, but has no explicit inertial elements. MuJoCo infers masses/COMs from its approximate geometry/default density. These differ materially from the URDF consumed by runtime gravity compensation. Matching five hinges cannot detect this divergence.

**Verified by MuJoCo model inspection:**

| Link | URDF mass / local COM | MJCF compiled mass / local COM |
|---|---|---|
| Shoulder pitch | 0.3 kg / (0,0,0) m | 0.113097 kg / (0,0,0) m |
| Shoulder roll | 0.3 kg / (0.05,0,0) m | 0.113097 kg / (0,0,0) m |
| Upper arm | 0.55 kg / (0,0,-0.18) m | 0.791681 kg / (0,0,-0.12) m |
| Forearm | 0.35 kg / (0,0,-0.14) m | 0.458149 kg / (0,0,-0.10) m |
| Hand | 0.12 kg / (0,0,-0.04) m | 0.108909 kg / (0,0,-0.03) m |

**Fix:** generate explicit MJCF inertials from the accepted URDF/CAD export and verify mass/COM/inertia/frame/joint equality numerically. Keep geometry-only approximations separate from physical properties. Record exporter/version, SolidWorks assembly/configuration, CAD file hashes, mass-property units and expected joint order. Test known poses against an independent gravity reference before using sim as commissioning evidence. No actual SolidWorks CAD geometry/mass inspection was performed here.

### T29 — CAD export and URDF conversion commands are success-reporting scaffolds

**Source:** [scripts/export-urdf.sh:12–29](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/export-urdf.sh#L12); [scripts/urdf-to-mjcf.sh:17–28](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/urdf-to-mjcf.sh#L17); `docs/decisions/0003-simulation-testing.md`.

These scripts explicitly describe manual workflows. Export returns `ok` when an existing URDF is present and mesh directories exist. Conversion performs no conversion; it runs a hinge-count test. The conversion instructions still call the production model a 2-DOF prototype. A developer returning after a break can reasonably mistake these command names/success messages for a completed refresh and carry stale geometry/inertials forward.

**Fix:** retain manual mode but report it clearly as “manual export required; existing assets only checked,” and update 5-DOF model descriptions. A real export/conversion path should emit provenance, compare expected joints/units, validate meshes and inertials, and fail on stale/missing output. Use hashes/manifests so the UI and deploy manifest can identify the model actually used. Local SolidWorks source binaries remain ignored; maintain versioned offline backup/sync under the requested `J:\code` home rather than treating Git bundles as CAD backup.

### T30 — Partial gravity-preview vectors silently evaluate the zero pose

**Source:** [bins/motor-repl/src/main.rs:412–438](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/motor-repl/src/main.rs#L412); [tools/marengo-pi-mcp/src/tools/readonly.ts:81–93](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/readonly.ts#L81); [src/harness/index.ts:288–324](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/harness/index.ts#L288).

The CLI accepts supplied angles only if their count reaches the configured joint count; otherwise it replaces the entire vector with zeros. Default MCP/historical weighted harness still send two angles. With the current five-joint master this means a caller asks for a nonzero gravity preview but receives zero-pose torques without warning; excessive angles are also silently truncated. Historical 2-DOF harness logic no longer maps the master order reliably.

**Verified:** exact CLI branch and captured real MCP handler command show two supplied angles for the five-joint model. No CLI CAN initialization or robot access was run.

**Fix:** allow no angles as an explicit zero default, require exactly N angles otherwise, or accept named joint angles with validated mapping. Query joint order/count from the accepted model for MCP/harness instead of hardcoding a dual-pitch vector. Test empty/full/partial/excess vectors, invalid numbers and the current five-joint order.

## Dependency findings and completed lock mitigations

### T31 — Node dependency audits reveal production-tool and Consul advisories

**Baseline source:** [tools/marengo-pi-mcp/package-lock.json:440,542,879,1020,1067,1257](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/package-lock.json#L440); [tools/compound-auto-learn/package-lock.json:39,65,1666](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/compound-auto-learn/package-lock.json#L39); [consul/package-lock.json:3075,3240,3261,4875,4890,5260,5449,5679](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/package-lock.json#L3075).

Audits were queried during this review. Counts are vulnerable *package records*, not independently reproduced exploits. Reachability depends on the APIs used: the current Pi MCP is stdio and does not expose an Express/Hono web server; Consul is a client SPA, not server-rendered React Router. Do not label every package advisory as an exploitable public endpoint.

**Pi MCP production audit:** 6 package records, **3 high, 2 moderate, 1 low**.

| Locked package | Version | Reported severity | Advisory examples / compatible repair target |
|---|---|---|---|
| fast-uri | 3.1.2 | High | URI authority/normalization issues; [GHSA-qw65-cvwx-89v3](https://github.com/advisories/GHSA-qw65-cvwx-89v3); select >=3.1.7 |
| hono | 4.12.23 | High | CORS and several parser/adapter issues; [GHSA-88fw-hqm2-52qc](https://github.com/advisories/GHSA-88fw-hqm2-52qc), [GHSA-g6gw-c38x-mqfc](https://github.com/advisories/GHSA-g6gw-c38x-mqfc); select >=4.13.5 |
| ip-address | 10.2.0 | High | Address classification/normalization bypasses; [GHSA-mwp4-54f8-5fhr](https://github.com/advisories/GHSA-mwp4-54f8-5fhr), [GHSA-rpw4-54j3-4h4q](https://github.com/advisories/GHSA-rpw4-54j3-4h4q); select outside <=10.5.0 |
| @hono/node-server | 1.19.14 | Moderate | Windows static path traversal; [GHSA-frvp-7c67-39w9](https://github.com/advisories/GHSA-frvp-7c67-39w9); >=1.19.15 |
| qs | 6.15.2 | Moderate | Parser denial of service; [GHSA-4mjr-xmp4-gh2g](https://github.com/advisories/GHSA-4mjr-xmp4-gh2g); >=6.16.0 |
| body-parser | 2.2.2 | Low | Invalid limit can disable enforcement; [GHSA-v422-hmwv-36x6](https://github.com/advisories/GHSA-v422-hmwv-36x6); >=2.3.0 |

The current MCP SDK's compatible dependency ranges permit patch/minor repairs for these transitives. Regenerate that lock deliberately, run its handler/transport tests, and re-audit production dependencies. No Pi MCP dependency lock was changed by this reviewer.

**Compound production audit:** 3 package records, **1 high and 2 moderate**, from `@cursor/sdk@1.0.24` → `@connectrpc/connect-node@1.7.0` → `undici@5.29.0`. Undici has several HTTP/WebSocket advisories, including [fragment-count denial of service](https://github.com/advisories/GHSA-vxpw-j846-p89q). The registry offers SDK **1.0.34**, outside the reported affected SDK range, with a different dependency transport graph. Select and test that compatible candidate against the tool's API/behavior tests; do not force an incompatible Undici major into connect-node 1.x. No API key or Agent.prompt call was used. The compound lock remains unchanged.

**Consul full audit at baseline:** 8 package records, **2 high and 6 moderate**. The root task authorized a compatible lock-only refresh in Linux Compose, using no force flag and no major/direct dependency edits. The root package metadata and package.json remained unchanged; no packages were added or removed. Selected updates:

| Package | Baseline → refreshed | Primary advisory support |
|---|---|---|
| browserslist | 4.28.2 → 4.29.3 | [Unbounded query-cache growth; patched 4.28.7](https://github.com/advisories/GHSA-c83g-rgw3-j3cx) |
| undici (jsdom) | 7.29.0 → 7.30.0 | [TLS options loss; patched 7.29.1](https://github.com/advisories/GHSA-w293-vg96-wgc3), plus DoS/HTTP/cookie advisories in audit JSON |
| baseline-browser-mapping | 2.10.31 → 2.11.26 | [Invalid-input process exit; patched 2.11.0](https://github.com/advisories/GHSA-w5vr-8v7q-w6rv) |
| vitest / @vitest/mocker | 4.1.9 → 4.1.11 | [Redirect-mock path traversal](https://github.com/advisories/GHSA-82fw-gwwq-j7x9) |
| nested fflate (three-stdlib) | 0.6.10 → 0.6.11 | [Malformed ZIP64 infinite loop](https://github.com/advisories/GHSA-px8p-9vwx-vf98) |
| react-router-dom / react-router | 6.30.4 → 6.30.6 | [v6 redirect/XSS backport](https://github.com/advisories/GHSA-jjmj-jmhj-qwj2) |

After that refresh, `npm audit --audit-level=high` exits **0**, with **0 high/critical and 2 moderate package records** remaining for React Router. The [backslash navigation advisory](https://github.com/advisories/GHSA-wrjc-x8rr-h8h6) remains applicable to v6; the [SSR hydration advisory](https://github.com/advisories/GHSA-337j-9hxr-rhxg) requires SSR/hydration behavior Consul does not currently use. Plan/test a Router 7.18+ migration and keep navigation inputs constrained; do not claim a zero-advisory lock. The root task owns full post-refresh test verification.

### T32 — Baseline Rust lock contains patched vulnerabilities and maintenance warnings

**Source:** `Cargo.lock:92,934,1937,2396,2424` at baseline; root gate's audit output and `rust-advisory-fix.log`.

| Package at baseline | Finding | Recommended repair / root status |
|---|---|---|
| h2 0.4.14 | [RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258), unbounded empty DATA queue | Patched >=0.4.16; root updated to 0.4.16 |
| rustls 0.23.40 | [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285), TLS handshake encryption-boundary acceptance | Patched >=0.23.45; root updated to 0.23.45 with its compatible crypto transitives |
| anyhow 1.0.102 | [RUSTSEC-2026-0190](https://rustsec.org/advisories/RUSTSEC-2026-0190), mutable downcast after context can violate borrow rules | Patched >=1.0.103; root updated to 1.0.103; no repository downcast_mut call found |
| paste 1.0.15 | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436), unmaintained | Replace/upgrade owning dependency path where feasible; maintenance warning remains |
| rustls-pemfile 2.2.0 | [RUSTSEC-2025-0134](https://rustsec.org/advisories/RUSTSEC-2025-0134), unmaintained | Migrate direct PEM parsing to rustls-pki-types::pem::PemObject and upgrade dependent wrappers; warning remains |

Baseline local `just check` can still return green because audit findings are warnings outside CI mode. The root task patched h2/rustls/anyhow within existing Cargo requirements and reports the strict CI-mode gate passing against the final lock, with the two unmaintained-package warnings remaining. Preserve the distinction between a successful compile/test gate, an advisory scan, and accepted maintenance warnings. Add an explicit release dependency policy rather than relying on a locally nonfatal audit.

## Architecture recommendations

1. **Keep one control owner.** UI, MCP and CLI should all submit commands to the long-lived Pi process through a typed IPC interface. A verified owner lease, fresh safety state and explicit stop outcome belong inside that module. A motion command must not spawn a second CAN controller or infer stop from a process string. This repair addresses T05/T07/T12 and the main control review's one-shot CLI problems together.
2. **Create one configuration authority.** Own live state, staged disk generation, all persistence acknowledgments, merge policy, activation/restart, and recovery inside the Pi. Deployment/Set Limits/local mirror/MCP sync should use that interface. An acknowledged file write, a model import, and a complete safe activation are distinct outcomes. This is the shared seam for T02/T03 and the gateway's lost-write/concurrent-URDF defects.
3. **Separate installed code from writable state.** Runtime account may update narrow config/calibration/log data but cannot replace release binaries/scripts or root helpers. Release installation is a transactional administrator operation. Avoid recursive ownership changes over mixed code/state trees.
4. **Replace shell-string orchestration with typed operations.** Carry argv, checked joint/config identities, budgets, source SHA and structured results. Keep raw arbitrary script capability separate from normal read/motion tools and require its own narrow explicit scope. Tests should exercise generated scripts, not just inspect strings.
5. **Treat generated plans as proposals.** Complete/fresh hardware context, accepted predecessor state, exact planner materialization and backend verification are required before Apply/Run. The Agent-generated JSON should not be the source of safety facts. Keep schema-only working directory separation and token/body/rate limits already present in Compound.
6. **Give each host one source tree.** Native Windows `J:\code\marengo` includes local CAD under that root; Mac uses its native clone, with portable Node/tooling and Docker Linux checks. Linux remains the robot/runtime/container platform, without an extra authoritative WSL software checkout. Named container caches avoid polluting source or copying OS-specific dependency directories between hosts. Add platform smoke rather than claiming untested Mac parity.
7. **Track model provenance and backup CAD explicitly.** Git tracks URDF/meshes/manifests, while ignored SolidWorks binaries need an independent versioned backup. Tie exports to assembly configuration, file hashes, measurement units and exporter versions. Preserve recovered older CAD/model variants without making them the current five-joint runtime by accident.
8. **Build simulation around the real seams.** Generate a physically equivalent plant, then exercise production Berthier/Davout and a simulated bus with independent oracles. UI/operator/tooling integration should consume that same safety/configuration contract. Keep future full humanoid/Jetson/Isaac work labelled as future scope; a five-hinge smoke is not a commissioning signoff.
9. **Make audit incompleteness visible.** Daily scan errors or missing GitHub/scanner evidence should produce an unknown/failed status. The current time-window audit can become clean simply because a finding aged out of its scan window; do not use automatic issue closure as proof a durable defect was fixed. Keep an explicit reviewed backlog with reproduction/acceptance criteria.

## Verification and reproducibility

Artifacts are preserved under `J:\code\marengo-migration-backup-20260929`:

- `tooling-review-repro.py`: research cache/arxiv/scrape, daily-audit false positives, missing YAML-key preservation checks. Rerun against locked review-main Python environment; all described failures reproduced offline.
- `tooling-review-repro.mjs`: parsed 5-second timeout, negative Wave wait, harmless substitution and raw joint interpolation. Handler capture only; no SSH.
- `tooling-more-repro.mjs`: read-only path substitution acceptance, Bash syntax validation and MCP failure-status classification.
- `tooling-limit-cors-repro.mjs` plus `tooling-noop.exe`: foreign-origin local writer request; real writer replaced by no-op, server stopped afterward.
- `tooling-auto-learn-repro.ts`: incomplete joint context and empty-prior acceptance. No Agent/API call.
- `tooling-permission-repro.py`: temporary Linux helper replacement proof without executing a helper.
- `tooling-sim-repro.py`: actual compiled production MuJoCo inertial comparison; additional production smoke passed nq5/nv5 with no actuators.
- `npm-audit-pi-production.json`, `npm-audit-compound-production.json`, `npm-audit-consul.json`: complete exact advisory records/versions queried for this review.
- `consul-lock-before-security-refresh.json`, `consul-security-lock-refresh.log`, `npm-audit-consul-after-refresh.json`: compatible root lock refresh and resulting high-severity gate.
- `rust-advisory-fix.log` and root full-check logs: root's independent Rust mitigation/gate evidence.

Behavior suites rerun during this resumed tooling review: **13 Compound tests**, **24 Python tests plus 12 subtests** (research/daily audit/preservation), **5 Rust sim-harness tests**, **5 deploy-revision assertions**, **4 Consul-dist assertions**, and **2 Consul rebuild assertions** passed. The deploy-job shell contract attempted in a read-only linked-worktree container could not access its Windows absolute Git worktree metadata or write Cargo target; its environment failures are not reported as product defects. Root's full workspace gate covers the underlying deploy crate separately.

The root reviewer reports the final strict gate passing **582 Rust tests (10 ignored), 355 Consul tests, 72 Pi MCP tests, clippy, deny, audit and aarch64 cross-build**, plus `just sim-check`. Local vCAN is unavailable in the Docker Desktop kernel; root separately reviewed passing CI vCAN evidence. Two Rust maintenance warnings and two moderate React Router package records remain, alongside the open tooling dependency findings above.

Build/test success and these source proofs do not verify the current Pi's live permissions, motor ownership, service state, physical reference, attached hardware, SolidWorks geometry or real motion. No such current-state claim is made. The main review owns the final merge/status reconciliation; this appendix preserves the baseline defects and the specifically verified lock mitigations.
