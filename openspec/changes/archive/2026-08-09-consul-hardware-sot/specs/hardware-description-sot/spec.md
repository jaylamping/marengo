# hardware-description-sot Specification

## Purpose

One durable hardware description: root `config/{robot,motors,control,homing}.yaml` and `assets/urdf/marengo.urdf`. Runtime boot, deploy, and operator edits MUST converge on these master paths. Completeness v1 warns only; it MUST NOT block boot, import, activation, or Enable.

## Requirements

### Requirement: Master config tree

The system MUST treat `config/{robot,motors,control,homing}.yaml` as the sole durable YAML description. Pi durable layout MUST be `/opt/marengo/config/` with the same four files. `MARENGO_CONFIG_DIR` defaults MUST resolve to the master tree. Day-1 cutover MUST seed root YAML from collapsed `config/bringup/arm_4dof_right/`.

#### Scenario: Bench boots from master paths

- GIVEN Pi env points at `/opt/marengo/config/`
- WHEN marengo-pi starts after cutover
- THEN it loads the four root YAML files without reading `config/bringup/*` as runtime SoT

#### Scenario: Bringup is not runtime SoT

- GIVEN a request targets `config/bringup/*` or gateway `/config/profiles*`
- WHEN runtime or gateway handlers run post-cutover
- THEN bringup paths are not used as authoritative config and profile registry routes are retired

### Requirement: Master URDF

The active kinematic description MUST be `assets/urdf/marengo.urdf`. Day-1 cutover MUST promote `arm_4dof_right.urdf` to `marengo.urdf`. Slice URDFs and placeholders MUST move to archive-only storage, not alternate active SoTs.

#### Scenario: Single active URDF

- GIVEN cutover is complete
- WHEN Davout or dynamics load URDF at boot
- THEN the loaded file is `marengo.urdf` under `assets/urdf/`

### Requirement: Completeness v1 warn-only

Gateway or `marengo-config` MUST compute completeness for mass/COM, kinematics, hard limits, and config coverage. Results MUST be exposed to Consul as warnings. HTTP responses and UI flows MUST NOT hard-block on incomplete status.

#### Scenario: Incomplete hardware still enables

- GIVEN completeness reports missing mass on a configured joint
- WHEN the operator attempts Enable with otherwise valid limits
- THEN Enable is not blocked solely by completeness warnings

#### Scenario: Consul renders warnings only

- GIVEN completeness returns one or more warning codes
- WHEN the Hardware workspace loads
- THEN warnings are visible and no modal or route guard blocks navigation or import solely for completeness

### Requirement: ADR 0012 runtime limit SoT

Live numerical limits MUST remain in-memory on the Pi (Davout/Berthier aggregate). YAML and URDF on disk are boot seed and async write-behind. HTTP Apply success MUST require Pi ACK with `persist_status`, not publish alone.

#### Scenario: Live snapshot precedes disk

- GIVEN a Durable Set Limits ACK updated in-memory hard bounds
- WHEN Consul reads actuator limit snapshot before write-behind completes
- THEN displayed hard bounds match the live snapshot, not stale disk YAML

### Requirement: ADR 0017 expand-only URDF hard envelope

Bench Set Limits Apply MUST expand-only widen URDF `<limit>` hard when taught hard exceeds current URDF hard. Soft bounds MUST use ADR 0009 inset in `control.yaml`. Shrink MUST remain `URDF ∩ motors.yaml` bench. Persist MUST be Durable only when URDF-first write-behind and YAML succeed together.

#### Scenario: Apply widens URDF hard

- GIVEN taught hard max exceeds current URDF hard for a joint
- WHEN Set Limits Apply succeeds with `persist_status: durable`
- THEN in-memory and on-disk URDF hard upper bound is widened and Enable no longer trips solely on the old URDF clamp

#### Scenario: Expand-only ratchet

- GIVEN taught hard is below current URDF hard
- WHEN Set Limits Apply runs
- THEN URDF hard is not narrowed; effective hard remains URDF ∩ bench

### Requirement: No alternate description SoTs

The system MUST NOT create durable alternate URDF or YAML trees for limb subsets, profiles, or imports. Ephemeral views MAY filter master data in memory only.

#### Scenario: Subset does not fork SoT

- GIVEN the operator views a 4-DOF right-arm subset
- WHEN the view unloads
- THEN no new durable URDF or YAML SoT was written for that subset
