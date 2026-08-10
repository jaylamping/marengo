# live-hardware-telemetry Specification

## Purpose

Read-only live view of master inventory with runtime enrichment. Replaces `/subsystems` Inventory commissioning surface; no durable edits or energize commands.

## Requirements

### Requirement: Telemetry route

Consul MUST provide `/telemetry` as the live hardware table for the full master inventory. The view MUST show joint/actuator rows with live RobotState enrichment (position, velocity, effort, temperature, facets when wire available) and MUST NOT mutate runtime or disk SoT.

#### Scenario: Telemetry loads master rows

- GIVEN gateway publishes RobotState for configured joints
- WHEN the operator navigates to `/telemetry`
- THEN all master inventory rows render with latest live fields

#### Scenario: Telemetry is read-only

- GIVEN the operator is on `/telemetry`
- WHEN they inspect row actions and modals
- THEN no Set Limits, Set Zero, Enable, scope edit, or go-to-zero commissioning controls are offered

### Requirement: Subsystems redirect

`/subsystems` MUST redirect to `/telemetry` (permanent or client-side equivalent). Sidebar and deep links MUST target Telemetry, not Subsystems/Inventory.

#### Scenario: Legacy subsystems URL redirects

- GIVEN a bookmark to `/subsystems`
- WHEN the operator opens it
- THEN they land on `/telemetry` without an Inventory commissioning UI

### Requirement: No commissioning commands on Telemetry

Telemetry MUST NOT expose gateway commands for limit patch, set_zero, enable, home/homing_complete, or commissioning-scope mutation. Read-only display of limits and live snapshots is permitted.

#### Scenario: Limit display without Apply

- GIVEN live limit snapshot differs from disk
- WHEN the operator views a row detail on Telemetry
- THEN limits are shown read-only with copy directing calibration to `/hardware`

### Requirement: Actuators route retirement

The `/actuators` route and sidebar entry MUST be removed or unreachable. Live actuator detail belongs under Telemetry (read-only) or Hardware (commissioning).

#### Scenario: Actuators nav absent

- GIVEN cutover is complete
- WHEN the operator views primary sidebar navigation
- THEN Actuators is not listed and `/actuators` does not render a commissioning shell

### Requirement: Wire-gated facet display

When Chappe homing or drive fields are absent (pre-wire client or old runtime), Telemetry MUST NOT fabricate Ready/Active from localStorage. It SHOULD show neutral/unknown facet placeholders until wire fields are present.

#### Scenario: Old runtime without homing_state

- GIVEN RobotState joints lack `homing_state`
- WHEN Telemetry renders Reference facet
- THEN it shows unknown/not available, not Ready from browser storage

### Requirement: Automated verification

Consul tests MUST assert Telemetry route render, `/subsystems` redirect, absence of commissioning actions, and Actuators retirement. Phase gate: `cd consul && npm test` passes for UI changes in this domain.

#### Scenario: Vitest telemetry suite

- GIVEN telemetry and redirect tests for this change
- WHEN `cd consul && npm test` runs
- THEN redirect and read-only action assertions pass
