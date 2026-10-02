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

All nine actual-installer tests pass in a disposable root Docker fixture: the
seven existing contracts plus two new methods covering preserved aliases and
four refused target cases. Service/process controls are substituted; filesystem,
ownership, accounts, rsync and sudoers validation are real. The preceding seven
methods remain unchanged. Full primary gate, independent review, mutation and
exact PR/main delivery are pending; no qualification from those gates is claimed.
The baseline failure and candidate log are retained under
J:/code/marengo-migration-backup-20260929/batch33-pi-install-log-links.

T01 remains partial and T02/T03 remain open; this small compatibility repair
does not implement atomic versioned activation or fail-closed taught-limit
preservation. Keep all 102 findings/eight maintenance tasks and 26 verified,
13 partial,63 open. Preserve calibration, taught limits, model assets, logs,
CAD, existing branches/worktrees, Wave sign-off and the paused automation.

Pi update is authorized. The owner has confirmed motor power is on and approved
powered movement testing. Motor power is required for controlled movement;
the earlier power-off request concerned the software restart only. Before
activation establish the arm's stable supported state, stopped drive state,
physical E-stop readiness and a concrete rollback. The repaired build still
lacks qualified physical reference acquisition, so it cannot yet authorize
normal right-arm output. Before each actual movement propose the joint, bounds,
duration/caps and stop procedure, require explicit confirmation and pass the
commissioning checks. Ten-minute silence leaves movement pending and permits
independent work only. No deployment or physical motor command has occurred.
