# Read first

The software is being rebuilt as a fresh, lean stack for the **whole robot**. Every change is for the finished robot, never
for the limb currently on the bench.

1. [hardware/SOFTWARE_TENETS.md](hardware/SOFTWARE_TENETS.md): the rules every change is judged by. They override every ADR
   and every older doc.
2. [hardware/SOFTWARE_PLAN.md](hardware/SOFTWARE_PLAN.md): what to build, in order.
3. [review/sw-foundation-20261108/RECONCILIATION.md](review/sw-foundation-20261108/RECONCILIATION.md): what to keep, rebuild
   or delete in the existing code, and the bench lessons to copy out first.
4. [hardware/STATUS.md](hardware/STATUS.md): current hardware facts.

The existing crates, bins, tools, ADRs and `AGENTS.md` files describe the **legacy** stack. Read them for facts and bench
lessons, not as patterns. Don't extend legacy code or fix its tests; build new crates beside it. Don't edit `hardware/`
(the hardware project owns it), except skeleton values the hardware side asks for.
