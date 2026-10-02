# Batch33: preserve runtime log aliases during Pi installation

The owner requested finishing and pushing the current software chunk, syncing
the Pi software, then basic right-arm movement tests. Batch32 is delivered as
PR246/main1d855df with all five final PR/main checks passing. Its source and
native software qualification remain bound to b18faf4; the delivery receipts
are recorded in evidence/batch32. No new reference-grant implementation started.

Read-only preflight on the owner's Pi found three ordinary file aliases in
/opt/marengo/var/log: bench-latest.log, candump-latest.log and
position-trace-latest.csv. Each resolves to its existing timestamped regular
file in that directory. The delivered installer rejects every nested symlink,
including these runtime-generated aliases, before updating anything.

The actual installer regression probe reproduces that refusal at d224cbd4 with
the three aliases, including absolute and relative targets. The repaired
installer permits file aliases directly in var/log only when their fully
resolved target is a regular file in the same directory. The installer never
copies privileged release content through these aliases. Directory, external,
missing and calibration targets still refuse before service or file changes;
the existing installed-code and directory guards remain in place.

Review found that command substitution stripped trailing newline bytes from a
resolved filename. A real c6aa099 probe demonstrated a directory alias being
accepted when a regular sibling had the stripped name. NUL-delimited resolution
now preserves the exact pathname. All ten actual-installer tests pass: the
seven existing contracts plus preserved aliases, four refused target cases and
the newline-directory regression. The preceding nine method bodies are unchanged.
Service/process controls are substituted; filesystem, ownership, accounts,
rsync and sudoers validation are real. The original seven methods remain
unchanged. Both independent production reviews clear source 0b2eb75 with zero
actionable findings. Its full primary passes: 861 Rust, one existing ignored,
374 Consul, ten installer tests, fmt/clippy/buf/deny/audit and fatal all-workspace
ARM release. The final production mutation omitting the resolved-target guard
fails three actual installer subcase assertions; the ten unchanged tests replay
successfully. See evidence/batch33/final-source-qualification.json for exact
source/probe/artifact bindings. Both final qualification metadata reviews clear 334a8a0 with zero findings.
Delivered as PR247/main b827176 with the same tree as final head a52eb02.
The four selected final PR jobs pass; sim is path-filtered and skipped for this
installer-only production delta. All five merged-main jobs pass, including sim
and fatal ARM. Both independent reviewers also clear the final recording delta
at a52eb02. See evidence/batch33/delivery-receipt.json.

The preceding c6aa099 primary pass skipped ARM by branch selection and remains
provisional historical evidence only. The baseline failure used its nine-method
probe; the positive alias method is unchanged in the final ten-method file.
Newline regression and final mutation are separately identified. Raw captures
remain under J:/code/marengo-migration-backup-20260929/batch33-pi-install-log-links;
committed captures normalize line endings, terminal escapes and trailing space,
with both raw and committed hashes recorded.

Read-only/user-directory Pi preparation retains the old revision and running
services. A verified release archive and online SQLite backup are preserved;
the backup integrity check passes, its application schema marker is version3,
and the new ARM log CLI opens an isolated copy successfully. Unchanged runtime
native qualification remains bound to batch32/b18faf4. This is preparation,
not installation or physical acceptance. A separate staged limit-preservation
preview retains all five taught hard/soft envelopes, motor identity and previous
URDF bounds; see evidence/batch33/pi-limit-preview-receipt.json.

T01 remains partial and T02/T03 remain open; this small compatibility repair
does not implement atomic versioned activation or fail-closed taught-limit
preservation. The optional harness can also leave bench-latest.json pointing to
absent metadata; its dangling alias remains refused and is separate remaining
compatibility work. The observed Pi has only the three existing-file aliases.
Keep all 102 findings/eight maintenance tasks and 26 verified,
13 partial,63 open. Preserve calibration, taught limits, model assets, logs,
CAD, existing branches/worktrees, Wave sign-off and the paused automation.

The owner subsequently returned and explicitly confirmed stable support, clear
workspace and physical E-stop within reach: “Yes—all three setup conditions are
ready; proceed.” This supersedes the earlier ten-minute setup timeout. Motor
power stays on under the owner's standing authorization.

The exact-main b827176 release is now installed on the Pi. Initial staging
stopped before installation because copied script modes and local release
artifacts failed the source-clean check. Git executable modes were restored;
a stale consul/dist was backed up and replaced with all99 verified UI files
because the installer selects it before www. The qualified installer then
completed with exit0, preserving the three valid log aliases. Pi/gateway are
active with zero automatic restarts in the recorded sample. Both binary hashes
and all214 installed manifest entries verify, using the independent taught-limit
preview for the three preserved files. All five joint envelopes, motor identity,
calibration registry and runtime environment are preserved. The installed Store
integrity check passes with schema3. The HTTPS served UI matches the verified
index. See evidence/batch33/pi-activation-receipt.json and its actual captures.

Live snapshots report Disabled and all five joints Unhomed. A passive bounded
50-frame capture contains ten reports from each motor; its installed CLI parser
passes. No enable, SetZero, target or movement test was commanded. Lower-arm yaw
still reports -4.13228655, outside the preserved taught envelope. Physical
reference acquisition and installed-owner clients remain unqualified; R2b2 is
the next software dependency before physical acquisition and bounded tests.
Owner setup approval clears deployment preparation, while each actual movement
still requires its own concrete joint/bounds/duration/caps/stop proposal, explicit
confirmation and qualified commissioning. Ten-minute silence authorizes only
independent work. T01 remains partial; T02/T03 and physical motion acceptance
remain open. Source/runtime backups and concrete rollback remain retained.
