<p align="center">
  <img src="../../docs/portraits/berthier.jpg" alt="Louis-Alexandre Berthier" width="420"/>
</p>

# berthier

**Berthier** — control.

Runs the realtime control loop: trajectory tracking, mode switching, and command output to motor drivers. Consumes planner setpoints from [Talleyrand](../talleyrand/) and respects limits enforced by [Davout](../davout/).
