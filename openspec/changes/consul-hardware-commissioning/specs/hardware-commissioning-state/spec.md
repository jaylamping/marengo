# hardware-commissioning-state Specification

## Purpose

Runtime truth for per-joint commissioning facets, anatomical aggregation, persisted commissioning scope, and scoped Enable. Replaces localStorage readiness and enable-time auto-homing with Chappe-published state from `marengo-homing` and Davout.

## Requirements

### Requirement: Per-joint commissioning facets

Each configured joint in master inventory MUST expose four facets derived from wire truth, not UI stores:

| Facet | Source | Operator label |
|-------|--------|------------------|
| Presence | CAN feedback / wired snapshot | Online / Offline |
| Reference | `JointState.homing_state == Verified` | Ready / Not ready |
| Drive | `JointState.drive_active` | Active / Not active |
| Health | `JointState.fault`, `JointState.out_of_limits` | Fault / OutOfLimits / OK |

Joint **Ready** MUST mean homing reference **Verified** only. UI MUST NOT infer Ready from localStorage, operator attestation alone, or enable attempts.

#### Scenario: Ready follows Verified on wire

- GIVEN `JointState.homing_state` is not Verified for joint `right_shoulder_pitch`
- WHEN Hardware renders that joint row
- THEN the Reference facet shows Not ready regardless of prior UI state

#### Scenario: Presence requires feedback

- GIVEN an actuator is configured but has no live CAN feedback
- WHEN facets are computed
- THEN Presence is Offline even if disk config marks it wired

### Requirement: Badge priority

When multiple facet states compete for a single badge, the system MUST apply this priority (highest wins): **Fault > OutOfLimits > Offline > Active > Ready > Online**.

#### Scenario: Fault beats Active

- GIVEN a joint is drive Active and reports a non-zero fault bitmask
- WHEN the joint badge is rendered
- THEN the badge class is Fault, not Active

#### Scenario: OutOfLimits beats Ready

- GIVEN `out_of_limits` is true and homing_state is Verified
- WHEN the joint badge is rendered
- THEN the badge class is OutOfLimits, not Ready

### Requirement: Joint to Limb to Robot aggregation

Facets MUST aggregate Joint → anatomical Limb → full-master Robot using the same priority order. Joint Ready MUST equal reference Verified. Limb Ready MUST require every built (online or motor-mapped) member of that anatomical group to be Ready; unbuilt Offline members MUST NOT block Limb Ready. Robot Ready MUST require every master actuated joint that is built to be Ready; unbuilt Offline inventory MUST NOT block Robot Ready. Commissioning scope MUST NOT redefine Robot Ready — scope only filters Enable targets.

#### Scenario: Unbuilt inventory does not block Robot Ready

- GIVEN all built master actuated joints are Verified with no Fault/OutOfLimits and remaining inventory limbs are unbuilt Offline
- WHEN Robot aggregation runs against full master inventory
- THEN Robot Ready is true

#### Scenario: Scope does not fabricate Robot Ready

- GIVEN effective commissioning scope is the right 4-DOF arm and those joints are Verified, but another built master joint is Unhomed
- WHEN Robot aggregation runs
- THEN Robot Ready is false

#### Scenario: Limb inherits worst joint in limb

- GIVEN one built joint in a limb is Faulted
- WHEN the limb badge is computed
- THEN the limb badge is Fault

### Requirement: OutOfLimits and Fault classification

v1 MUST set `out_of_limits` from homing verification failure or Davout limit enforcement. Safety failures that cannot be classified as OutOfLimits MUST surface as Fault (non-zero fault or Faulted homing_state).

#### Scenario: Verification limit breach maps OutOfLimits

- GIVEN homing verification rejects a joint for exceeding hard limits
- WHEN RobotState is published
- THEN that joint has `out_of_limits = true` and homing_state Faulted or equivalent fault surfacing

#### Scenario: Unclassified safety failure maps Fault

- GIVEN Davout reports an active fault without an OutOfLimits classification
- WHEN facets are computed
- THEN Health resolves to Fault, not OutOfLimits

### Requirement: Commissioning scope persistence

Effective commissioning scope MUST persist at `/opt/marengo/var/commissioning-scope.yaml` as a versioned document listing joint names from master inventory. On boot, **effective scope** MUST be the intersection of persisted scope and `MARENGO_JOINT_SUBSET` (when set); the env subset is a ceiling, not a floor.

#### Scenario: Scope survives restart

- GIVEN the operator saved scope `{right_shoulder_roll, right_shoulder_pitch}`
- WHEN marengo-pi restarts and reloads scope
- THEN effective scope equals the intersection of saved joints and the startup ceiling

#### Scenario: No scope file means no persisted filter

- GIVEN no scope file exists and `MARENGO_JOINT_SUBSET` lists three joints
- WHEN effective scope is resolved at boot
- THEN there is no persisted commissioning-scope filter; the startup ceiling still limits which motors are loaded, and Enable uses the no-scope gate below

### Requirement: Scope apply and widen confirmation

Applying scope MUST require operator confirmation when the new selection **widens** energizable joints (adds joints not in the previous effective scope). Narrowing or equal replacement MAY apply without widen confirmation.

#### Scenario: Widen requires confirm

- GIVEN effective scope is `{roll, pitch}`
- WHEN the operator selects `{roll, pitch, yaw}` and clicks Apply
- THEN the UI MUST require explicit confirm before persisting

#### Scenario: Narrow applies without widen confirm

- GIVEN effective scope is `{roll, pitch, yaw}`
- WHEN the operator selects `{roll, pitch}` and clicks Apply
- THEN scope persists without a widen confirmation step

### Requirement: Scoped Enable all Ready in scope

Enable all Ready in scope MUST reuse `EnableRequest` semantics. The runtime MUST enable only joints that are simultaneously: in effective scope, homing Verified, not Faulted, and not OutOfLimits. Joints outside scope MUST NOT energize. If any targeted joint fails to reach Active, the runtime MUST disable all motors (partial failure is all-off).

#### Scenario: Enable skips Not ready in scope

- GIVEN effective scope includes joints A (Verified) and B (Unhomed)
- WHEN the operator triggers Enable all Ready in scope
- THEN only joint A is targeted and B remains disabled

#### Scenario: Enable excludes Fault and OutOfLimits

- GIVEN joint A is Verified and joint B is OutOfLimits, both in scope
- WHEN Enable all Ready in scope runs
- THEN only joint A is targeted

#### Scenario: Partial enable failure disables all

- GIVEN two Verified in-scope joints are targeted
- WHEN one joint fails to enable
- THEN all motors are disabled and the operator sees failure state

#### Scenario: Persisted scope enables without full Robot Ready

- GIVEN a persisted scope file yields a non-empty effective scope and those joints are Verified
- WHEN Enable all Ready in scope is requested
- THEN enable targets Verified in-scope joints without requiring full-master Robot Ready

#### Scenario: No scope file requires Robot Ready

- GIVEN no commissioning-scope file exists and loaded master joints are the enable candidates
- WHEN Enable is requested
- THEN the runtime MUST require full-master Robot Ready and MUST reject Enable when Robot Ready is false

### Requirement: Active never auto-restores

After restart or runtime rollback, drive Active MUST NOT be restored automatically. Davout MUST boot Disabled; returning to Active requires explicit operator Enable after scope and Ready checks.

#### Scenario: Cold boot is Disabled

- GIVEN motors were Active before power loss
- WHEN marengo-pi starts
- THEN all joints publish `drive_active = false` and SafetyState is not Active

### Requirement: No enable-time auto homing complete

The enable path MUST NOT call `set_homing_complete()` or equivalent bulk Verified transition. Verified MUST result only from explicit Set Zero / homing verification flows.

#### Scenario: Enable does not mark Unhomed Verified

- GIVEN a joint is Unhomed
- WHEN the operator enables another Verified in-scope joint
- THEN the Unhomed joint remains Unhomed on the wire

### Requirement: HomingComplete and Testing Home retired

`HomingComplete` commands and Testing **Home** (mark all Verified) MUST NOT be operator paths. Readiness MUST be established per joint via Hardware Set Zero and verification only.

#### Scenario: Testing Home absent

- GIVEN the operator is on `/testing`
- WHEN the page renders commissioning controls
- THEN no Home / Mark READY control is shown
