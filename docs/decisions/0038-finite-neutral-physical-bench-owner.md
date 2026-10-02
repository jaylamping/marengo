# ADR 0038: finite neutral physical bench owner

Status: accepted for neutral qualification implementation, October 2, 2026.

## Context

ADR0037's actual disabled home qualification passed on the unchanged five-joint
arm. Every Set Zero reply was fault-free Reset, positions were within 0.00018 rad
of zero, and timeout readbacks were 600. The operator confirmed mechanical home
and asked to proceed while leaving motor power on. A deliberate power-cycle
qualification is unavailable; neither timeout volatility nor timeout output-stop
behavior is established. Ordinary physical reference acquisition remains absent.

## Decision

Add a closed, finite **neutral-only** physical bench owner. It opens its own real
RuntimeBus from installed configuration. No caller-supplied transport, receipt,
history row, virtual realm or epoch can create it. It requires explicit supported
mechanical home, unchanged sign mapping, operator identity and neutral-enable
approval. Its profile is restricted to the five observed can0 devices/model,
direction/gear and firmware/MCU identities recorded in the October 2 evidence.

The owner performs fresh disabled qualification itself, then exclusively creates
and durably syncs an audit containing actual receipt and installed typed policy/
URDF. It checks fresh addressed home/timeout replies after durable completion,
then consumes that actual completion into private current reference for all five
joints. Permission binds this installed model/policy, stop generation and a
five-second owner lifetime. It uses no physical device epoch. Persisted audit is
history only and has no recovery/import/permission API.

The opaque owner exposes no mutable Supervisor or bus. Enable uses the ordinary
Davout Ready/Active FSM once. Before admission all drives must remain Reset. Only
one Enable session is allowed; every stop, fault, expiry or shutdown revokes its
permission. There is no automatic re-arm. After enable, immediate neutral MIT
and fresh addressed position/timeout reads pass through the shared operational
hazard consumer before the finite tick sequence.

Berthier owns a 200 Hz, 500 ms neutral sequence. Davout independently refuses any
nonzero MIT field or incomplete/repeated joint batch at this interface, checks
home drift and all ordinary feedback/limits/fault/watchdog gates, and performs
all-address stop on every outcome. Blocking acquisition, audit sync and parameter
queries happen outside this tick sequence. Reporting remains Off throughout.

## Consequences

This permits an explicitly supported neutral enable qualification without
pretending unmeasured timeout behavior is a reset witness. It authorizes no
nonneutral torque, stiffness, damping, trajectory or elevated hold. Full motion
testing still requires a separately reviewed finite motion profile and its
physical evidence; this neutral owner cannot be used to bypass that restriction.
Caller application identity has no role in admission.

Actual physical support, E-stop function, exclusive CAN ownership and successful
stop acceptance remain operator/hardware evidence rather than software claims.
The main installed runtime stays stopped and its ordinary constructors do not
gain reference from this bench audit.
