# hardware-operator-workspace Specification

## Purpose

Consul `/hardware` is the sole operator workspace for durable hardware description edits, import with explicit resolution, completeness warnings, and bench Set Limits. Inventory MUST NOT retain a durable Set Limits path.

## Requirements

### Requirement: Hardware route and table-first layout

Consul MUST provide a `/hardware` workspace listing actuators and joints in a table-first layout. An optional Table/3D toggle MAY switch views; table remains the default (3D-as-default is out of scope).

#### Scenario: Default table view

- GIVEN the operator navigates to `/hardware`
- WHEN the workspace loads without toggling view
- THEN the table layout is shown with actuator/joint rows

#### Scenario: Optional 3D toggle

- GIVEN the workspace is loaded
- WHEN the operator selects 3D view
- THEN a kinematic visualization of the master model is shown without changing durable SoT

### Requirement: Unified settings sheet

Joint and actuator edits MUST open a unified settings sheet for membership, limits display, and hardware metadata. Disk snapshot fields MUST be labeled separately from live limit snapshot per ADR 0012.

#### Scenario: Live vs disk limits distinguished

- GIVEN live actuator limit snapshot differs from disk `GET /config/snapshot`
- WHEN the settings sheet opens
- THEN Range/hard display prefers live snapshot and disk is shown as boot seed only

### Requirement: Import wizard with field-level resolve

Hardware MUST provide an import wizard: upload contributor → review conflicts → pick winners for kinematics-critical fields → Accept. Accept MUST make resolved fields active (equivalent to successful activate on the management API). Cancel MUST leave active master unchanged.

#### Scenario: Accept activates resolved import

- GIVEN the operator resolved all required import conflicts
- WHEN they click Accept
- THEN active master reflects resolved fields and staging/archive rules from the management API

#### Scenario: Cancel preserves active

- GIVEN an in-progress import with unresolved or resolved preview
- WHEN the operator cancels
- THEN `marengo.urdf` and root YAML remain as before the wizard opened

### Requirement: Completeness warning badges

The workspace MUST render completeness warnings as non-blocking badges or banners. Warnings MUST NOT disable Enable, import Accept, Set Limits Apply, or route entry.

#### Scenario: Warnings do not gate actions

- GIVEN completeness warnings are present
- WHEN the operator clicks Accept or Apply on an otherwise valid action
- THEN the action is not disabled solely by warning badges

### Requirement: Exclusive durable Set Limits on Hardware

Durable bench Set Limits (live limit patch with write-behind per ADR 0012 and URDF expand per ADR 0017) MUST be available only from Hardware. Apply MUST require motors not ACTIVE, use existing gateway limit-patch ACK semantics, and surface `persist_status` (pending/durable/failed). Persist failure MUST show degraded banner without NeedsRestart.

#### Scenario: Set Limits on Hardware succeeds hot-reload

- GIVEN motors Disabled/Ready and a valid sweep session
- WHEN Apply on Hardware succeeds
- THEN live limits update, URDF expand-only applies when needed, and ACK reports persist status without NeedsRestart dialog

#### Scenario: Set Limits blocked while ACTIVE

- GIVEN actuator state is ACTIVE
- WHEN the operator attempts Set Limits
- THEN start/Apply is disabled with motors-must-be-disabled guidance

### Requirement: Inventory durable Set Limits removed

Inventory MUST NOT expose a durable Set Limits Apply path to gateway limit patch or profile txn persist. Inventory MAY show read-only limit display from live snapshot. Any Inventory limit UI MUST NOT write motors, control, or URDF durably.

#### Scenario: Inventory cannot persist limits

- GIVEN the operator is on Inventory for an actuator
- WHEN they attempt an action that previously persisted Set Limits
- THEN no durable limit patch is sent and UI directs to Hardware for calibration

### Requirement: Telemetry read-only (no presets, no editable Range)

Telemetry (`/telemetry`) MUST be a read-only live inventory. It MUST NOT offer Assign preset / bench preset editing, editable Range cells that look like Set Limits, or identity Apply that writes local catalog overrides. Range MAY display live snapshot text; Set Limits Apply remains Hardware-only.

#### Scenario: No Assign preset on Telemetry

- GIVEN the operator is on `/telemetry`
- WHEN the device table renders
- THEN no Preset column assign control is offered

#### Scenario: Range is display-only on Telemetry

- GIVEN an actuator row shows a Range value
- WHEN the operator clicks the Range cell
- THEN no inline edit / fake save path opens; calibration is directed to Hardware

### Requirement: Automated verification

Consul tests MUST cover Hardware route render, warning-non-blocking behavior, Set Limits ACTIVE guard, and Inventory absence of durable persist. Phase gate: `cd consul && npm test` passes for UI changes in this change.

#### Scenario: Vitest regression suite

- GIVEN Hardware and Inventory test files for this change
- WHEN `cd consul && npm test` runs
- THEN new and updated component tests pass
