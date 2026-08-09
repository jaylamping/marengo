# Delta for hardware-operator-workspace

## ADDED Requirements

### Requirement: Master chrome without bringup presets

Consul chrome (site header, overview subtitle, sidebar labels, hook defaults) MUST reflect full master inventory only. Strings and defaults `bench_4dof`, `arm_4dof_right`, and profile-scoped bringup labels MUST NOT appear in operator-facing chrome or Testing defaults.

#### Scenario: Overview shows master context

- GIVEN cutover is complete
- WHEN the operator views the overview header
- THEN subtitle describes master inventory, not `arm_4dof_right · bench`

#### Scenario: Testing hooks default to master

- GIVEN a fresh Consul session without local overrides
- WHEN Testing motion hooks initialize joint selection
- THEN they derive from master inventory, not `arm_4dof_right` preset

### Requirement: Hardware owns commissioning commands

Hardware MUST be the sole operator surface for Set Limits, Set Zero, facet display, commissioning scope selection, and Enable all Ready in scope. Testing MUST retain motion, go-to-zero, and E-stop only; it MUST NOT offer Enable, Disable-as-commissioning-primary, or Home/Mark READY.

#### Scenario: Enable on Hardware only

- GIVEN the operator needs to energize Verified in-scope joints
- WHEN they search for Enable in the UI
- THEN Enable all Ready in scope is on `/hardware`, not `/testing`

#### Scenario: Set Zero on Hardware

- GIVEN a joint needs mechanical reference capture
- WHEN the operator performs Set Zero
- THEN the control is available from Hardware settings, not Telemetry

#### Scenario: Testing retains motion and E-stop

- GIVEN the operator is on `/testing`
- WHEN the control bar renders
- THEN motion and E-stop are available and Enable/Home are absent

### Requirement: Hardware scope and facet UI

Hardware MUST render per-joint facets and badges per `hardware-commissioning-state` priority, show effective commissioning scope with apply/widen confirm, and expose Enable all Ready in scope with explicit failure feedback on partial enable.

#### Scenario: Scope editor on Hardware

- GIVEN master inventory lists ten joints and effective scope is four joints
- WHEN the operator opens scope controls on Hardware
- THEN they can narrow or widen selection subject to ceiling and confirm rules

#### Scenario: Facet badges on Hardware rows

- GIVEN wire publishes homing_state, drive_active, and fault fields
- WHEN Hardware table renders
- THEN each row shows the highest-priority facet badge for that joint

### Requirement: Inventory and Actuators retirement

Inventory (`/subsystems`) and Actuators (`/actuators`) commissioning workspaces MUST be retired in favor of Telemetry (read-only) and Hardware (commissioning). No route MUST retain durable Set Limits, Enable, or Mark READY flows outside Hardware.

#### Scenario: No Inventory Set Limits path

- GIVEN the operator follows sidebar to Telemetry
- WHEN they attempt limit calibration
- THEN UI directs them to Hardware; no durable limit patch is sent from Telemetry

## MODIFIED Requirements

### Requirement: Completeness warning badges

The workspace MUST render completeness warnings as non-blocking badges or banners. Warnings MUST NOT disable import Accept, Set Limits Apply, scope Apply, Enable all Ready in scope, or route entry.
(Previously: warnings also listed Enable without distinguishing scoped Enable on Hardware.)

#### Scenario: Warnings do not gate actions

- GIVEN completeness warnings are present
- WHEN the operator clicks Accept, Apply scope, or Enable all Ready in scope on an otherwise valid action
- THEN the action is not disabled solely by warning badges

### Requirement: Exclusive durable Set Limits on Hardware

Durable bench Set Limits (live limit patch with write-behind per ADR 0012 and URDF expand per ADR 0017) MUST be available only from Hardware alongside Set Zero. Apply MUST require motors not ACTIVE, use existing gateway limit-patch ACK semantics, and surface `persist_status` (pending/durable/failed). Persist failure MUST show degraded banner without NeedsRestart.
(Previously: did not co-locate Set Zero explicitly on Hardware commissioning surface.)

#### Scenario: Set Limits on Hardware succeeds hot-reload

- GIVEN motors Disabled/Ready and a valid sweep session
- WHEN Apply on Hardware succeeds
- THEN live limits update, URDF expand-only applies when needed, and ACK reports persist status without NeedsRestart dialog

#### Scenario: Set Limits blocked while ACTIVE

- GIVEN actuator state is ACTIVE
- WHEN the operator attempts Set Limits
- THEN start/Apply is disabled with motors-must-be-disabled guidance

### Requirement: Inventory durable Set Limits removed

Telemetry (formerly Inventory) MUST NOT expose a durable Set Limits Apply path to gateway limit patch or profile txn persist. Telemetry MAY show read-only limit display from live snapshot. Any Telemetry limit UI MUST NOT write motors, control, or URDF durably.
(Previously: referred to Inventory route; now Telemetry read-only successor.)

#### Scenario: Telemetry cannot persist limits

- GIVEN the operator is on Telemetry for an actuator
- WHEN they attempt an action that previously persisted Set Limits from Inventory
- THEN no durable limit patch is sent and UI directs to Hardware for calibration

### Requirement: Automated verification

Consul tests MUST cover Hardware route render, facet/scope/Enable UI, warning-non-blocking behavior, Set Limits ACTIVE guard, Telemetry read-only retirement, master chrome strings, and absence of Testing Enable/Home. Phase gate: `cd consul && npm test` passes for UI changes in this change.
(Previously: covered Inventory absence of durable persist only.)

#### Scenario: Vitest regression suite

- GIVEN Hardware, Telemetry, and Testing test files for this change
- WHEN `cd consul && npm test` runs
- THEN new and updated component tests pass
