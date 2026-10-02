# ADR 0031: explicit resources for closed simulation construction

Status: accepted for software implementation, October 1, 2026.

The physical Pi test run reproduced a constructor test failure because the
shared configuration resolver selected `/opt/marengo/config` ahead of the
simulation's copied repository root. A fixture that disabled diagnostic output
therefore still transmitted type-24 frames through its in-memory bus. The same
precedence can hide invalid fixture configuration and select installed history.

The closed `Supervisor<SimulationBus>::from_simulation*` and
`ControlLoop<SimulationBus>::from_simulation*` constructors load configuration
from the supplied root's `config/` directory. Controller dynamics and supervisor
initialization use that same choice. They share the physical initializer's
validation, reference, admission, stop and output logic. Configuration selection
does not grant physical authority or add a transport conversion.

Ordinary `from_repo*` constructors retain their documented environment and Pi
installation precedence. Simulation's existing explicit calibration-path
constructor continues to ignore the calibration environment override; the
default constructor retains that override. Tests isolate it in child processes.

Verification must run real public constructors with a competing configuration
directory, qualify invalid copied resources as errors, and retain the ordinary
constructor as a positive control for runtime configuration precedence. Child
process environment changes avoid mutating the parallel test runner. Pi tests
still use an in-memory bus and do not establish physical commissioning.
