# Batch29 — immutable installed helpers and unprivileged enqueue state

Baseline: `4b0d287004260c5286c1a240e300c796a6a9ccfa`. Branch:
`codex/install-immutable-helpers`. This advances WP10 T01 using the actual
installer in disposable Linux containers. The original 102 finding IDs and
eight maintenance tasks remain intact. Independent review and delivery are
pending; T01 is not claimed complete.

The old installer root-owned two sudo helper files while leaving their
directories and installed binaries writable by the runtime account. The
canonical six-test fixture runs the actual installer with real accounts,
ownership, rsync and sudoers validation. Only process/service controls are
substituted. It discovers the actual helper targets from the generated sudoers
policy. Against unchanged baseline installer/enqueue sources it records 17
assertion failures: helper and release replacement, mutable ancestors, accepted
code symlinks, undeclared executable writes, and privileged writes redirected
through runtime state. No replacement program is executed. The unchanged
canonical fixture passes all six tests against the repaired source.

Privileged restart/enqueue helpers now install under
`/usr/local/libexec/marengo`, with root-owned immutable ancestors and validated
sudoers targets. Runtime code directories are root-owned and sealed before
copying and again before restart. Existing redirected code, revision paths and
helper paths are refused. Only config, assets and var retain declared runtime
group write access. Gateway, deployment library and Pi MCP default helper paths
follow the new location. Administrator-configured overrides remain available;
the deployment account remains administrator-equivalent.

The enqueue helper also stopped opening runtime-controlled lock and temporary
filenames as root. Its lock is under a root-only /run directory. JSON state
creation, descriptor permissions and atomic replacement run as the trusted
deployment user with an unpredictable exclusive temporary file. A controlled
service-boundary barrier lets the runtime UID prepare the legacy predictable
temporary symlink. Both root sentinel files retain their bytes and modes while
a valid enqueue reaches the recorded launch boundary. No worker is launched.

The required Docker gate invokes this guarded root fixture only inside a
disposable container. Native non-Docker checks report that this contract belongs
to the primary Docker gate. No host sudo policy is modified. Root testing is not
run on the Pi. Canonical source hashes and red/green logs are recorded in
`evidence/batch29/source-qualification.json`.

Preliminary v3 failed before its intended temporary-file assertion because the
legacy helper changed the fixture's custom runtime group to the production
group. Canonical v5 uses the production marengo runtime account and reaches both
sentinel assertions. An earlier CRLF baseline preparation error is also excluded
from behavioral evidence. Both remain preserved in the recovery directory.

The first broad Linux primary passed 814 Rust tests (one existing ignored),
Consul 361, Pi MCP 72, research 83, daily-audit 15 and fatal aarch64 release build.
Later fixture/sealing edits have focused six-test and shell-syntax qualification;
final source review and required CI remain pending. Native Pi client tests are
running only in the existing isolated validation tree. Installed revision is
still `4bc77ba605834fdec04b436daa4bec67bca84fbb`.

Remaining acceptance: the implementation plan also calls for an immutable
versioned release tree. Versioned staging and atomic activation are not added
by this batch and remain a T01/T02 dependency. T02's taught-limit preservation
and rollback defects are still open. Software evidence does not establish live
installation or physical acceptance. No deployment, service restart, physical
CAN test or motor movement occurs. Owner confirmation before each movement and
the ten-minute-away policy remain in force.

Independent review of 289813b found nested writable-state symlinks still
redirected root mkdir/chmod/chown. Standards also identified staged rsync modes
reopening code entries before the final seal. Canonical v7 reproduces four
assertion failures against that candidate: var/log, var/calibration and both
scripts/www copy windows. The observer runs real rsync and immediately attempts
runtime rename of a never-executed marker. Against the original baseline v7
records 21 failures; against the corrected source all seven tests pass unchanged.
Existing state directories are now sealed in parent-first order before any
privileged writes, redirects are checked again once entries are sealed, and
rsync forces root ownership/nonwritable modes. Declared runtime write access is
restored after privileged installation writes. Historical v5 qualification is
retained separately from the final v7 receipt. Re-review and exact-source gate
are pending. Pi native deployment/gateway tests passed 65; both Rust input
hashes match the canonical source. No live device installation occurred.

The broad Linux primary at 9b2dd50 passed the full seven-test installer suite
and all earlier gate totals. Final restoration now grants regular-file modes
while all state parents remain sealed, then grants directory modes in postorder.
Canonical v7 replays unchanged green after this ordering change. The recursive
chmod window was identified from [Coreutils9.1 source](https://github.com/coreutils/coreutils/blob/v9.1/src/chmod.c)
and [gnulib chmodat](https://github.com/coreutils/gnulib/blob/master/lib/openat.h);
it is source-level evidence, not an additional reproduced failure.
Final independent re-review is clear at `0867a9304d9ceff5e579601bf85bb39469b752ea`:
Standards has zero documented violations/actionable smells; Spec has zero
remaining actionable scoped findings. The separate review receipt retains the
earlier findings and remaining T01/T02 acceptance. Exact-head hosted primary
and delivery remain pending.
