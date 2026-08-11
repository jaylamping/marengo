# Delta for hardware-management-api

## ADDED Requirements

### Requirement: JointState commissioning fields on Chappe

`JointState` MUST publish commissioning fields on RobotState via Chappe: `JointHomingState homing_state` (mirroring `marengo-homing`), `bool drive_active`, and `bool out_of_limits`. Values MUST originate from marengo-pi/Davout/homing registry without a parallel HTTP-only homing snapshot.

#### Scenario: Verified homing on wire

- GIVEN homing registry marks joint `right_elbow_pitch` Verified
- WHEN RobotState is published
- THEN that joint's `homing_state` is Verified

#### Scenario: Drive active per joint

- GIVEN Davout has enabled joint A but not joint B
- WHEN RobotState is published
- THEN joint A has `drive_active = true` and joint B has `drive_active = false`

#### Scenario: OutOfLimits published

- GIVEN Davout or verification marks a joint out of limits
- WHEN RobotState is published
- THEN that joint has `out_of_limits = true`

### Requirement: Commissioning scope read and write

The gateway MUST expose commissioning scope CRUD backed by `/opt/marengo/var/commissioning-scope.yaml` on the Pi. **Read (`GET`) MUST NOT require** `x-marengo-log-token` (LAN-bench parity with `/config/snapshot` so Consul www without a baked secret can load scope). Read MUST return version, joint list, ceiling (`MARENGO_JOINT_SUBSET` when set), and effective scope (after ceiling intersection). **Write (`PUT`) and clear (`DELETE`) MUST require** the gateway log token. Write MUST validate joint names against loaded master inventory, persist atomically, and reject unknown joints.

#### Scenario: Read effective scope

- GIVEN persisted scope lists four joints and `MARENGO_JOINT_SUBSET` lists three of them
- WHEN the client GETs commissioning scope without a log token
- THEN the request succeeds and the response includes persisted list, ceiling, and effective intersection

#### Scenario: Mutations require log token

- GIVEN `MARENGO_GATEWAY_LOG_TOKEN` is configured
- WHEN the client PUTs or DELETEs commissioning scope without `x-marengo-log-token`
- THEN the request is rejected with 401 and disk scope is unchanged

#### Scenario: Write rejects unknown joint

- GIVEN master inventory has no joint `left_wrist_roll`
- WHEN the client PUTs scope including `left_wrist_roll`
- THEN the request is rejected with an identifiable error and disk scope is unchanged

#### Scenario: Clear scope

- GIVEN a persisted scope file exists
- WHEN the client DELETEs or clears scope per API contract
- THEN no persisted commissioning-scope filter remains and subsequent Enable uses the no-scope Robot Ready gate against loaded master joints

### Requirement: Scoped EnableRequest handling

`POST /command/enable` (Chappe `EnableRequest`) MUST implement Enable all Ready in scope when `enable = true` and a persisted scope file yields a non-empty effective scope: target set = effective scope ∩ {joints with homing_state Verified} excluding Faulted and OutOfLimits joints. Out-of-scope and Not-ready joints MUST NOT energize. When no scope file exists, Enable MUST require full-master Robot Ready and MUST target loaded master joints that are Verified, Online, fault-free, and in bounds. On partial failure, runtime MUST disable all motors before ACK/error return.

#### Scenario: Enable honors scope

- GIVEN effective scope is `{roll, pitch}` and yaw is Verified but out of scope
- WHEN enable true is requested from Hardware
- THEN yaw is not enabled

#### Scenario: Enable rejects all-off targets

- GIVEN no in-scope joint is Verified
- WHEN enable true is requested
- THEN enable is rejected or no-ops without energizing motors

#### Scenario: Partial failure all-off

- GIVEN two in-scope Verified joints are targeted
- WHEN one joint fails Davout enable
- THEN all motors end Disabled and response indicates failure

### Requirement: No enable-time set_homing_complete

Gateway and marengo-pi enable handling MUST NOT invoke bulk `set_homing_complete()` before or during enable. Homing transitions to Verified MUST remain explicit via Set Zero and verification APIs only.

#### Scenario: Enable does not bulk-verify

- GIVEN two joints are Unhomed
- WHEN enable true is requested for an unrelated scoped subset
- THEN neither Unhomed joint transitions to Verified on the wire

### Requirement: HomingComplete command retirement

Gateway MUST NOT expose operator `POST /command/home` → `HomingComplete` as a mark-all-ready path. Existing message types MAY remain for compatibility but MUST NOT be wired to Testing Home UI.

#### Scenario: Home endpoint inactive for operators

- GIVEN an authenticated client POSTs homing complete for all joints
- WHEN the commissioning cutover is deployed
- THEN the call does not mark all joints Verified without per-joint verification

## MODIFIED Requirements

### Requirement: Ephemeral subset contract

Subset endpoints MUST return filtered views of master config/URDF in memory or transient response bodies. They MUST NOT write subset-specific durable URDF or YAML paths. **`MARENGO_JOINT_SUBSET` at Pi boot MUST act as the commissioning-scope ceiling**; persisted scope MUST NOT expand energizable joints beyond that ceiling.
(Previously: did not relate env subset to commissioning scope ceiling.)

#### Scenario: Subset response is ephemeral

- GIVEN a request for a limb subset (e.g., right 4-DOF arm)
- WHEN the subset endpoint responds
- THEN the response filters master data and no new durable SoT path is created

#### Scenario: Scope cannot exceed ceiling

- GIVEN `MARENGO_JOINT_SUBSET` lists three joints
- WHEN the client PUTs scope with five joints
- THEN effective scope contains at most the three ceiling joints
