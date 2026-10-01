# Spec review of b23c82b...3fa7e13

Independent reviewer /root/batch17_spec found one evidence gap: the recorded
mutation hash lacked a frozen patch/source, preventing independent inspection
that the pre-reservation read releases its statement/autocommit transaction.
The exact stale-read-mutant.patch has now been preserved and its reconstructed
production bytes verified against the original mutation receipt SHA256.
Follow-up review of this correction is pending.

The implemented test otherwise meets the reviewed bounded contract: public Store
APIs, literal fixture, WAL/idle controls, actual contention, whole-child callback
count, independent commit observation, preservation/FTS/integrity, reopened Store,
reaped child and fixture removal before collected assertion. Evidence classification
is honest and no scope creep was found. Required Linux gate/CI/delivery remain pending.
