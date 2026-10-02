# Batch31: gateway access and runtime credentials (in progress)

Baseline main `a5c4cdb6448c5a0164ebbeffb1c82eb09ab0c266`; branch
`codex/gateway-runtime-access`, worktree
`J:/code/marengo-worktrees/gateway-runtime-access`. ADR0033 records the contract.

Frozen original-public HTTP v2 records11 refused-request failures and2 positive
controls. The same13 cases replay unchanged green with authentication/Origin
before publication or subscription. HTTP and HTTPS use one immutable startup
policy with control, calibration, configuration, management and sensitive-read
capabilities. Legacy configured log credentials retain documented administrator
compatibility; scoped roles do not gain other capabilities. Unset/invalid
credentials fail closed. Missing Origin is permitted for authenticated CLI
clients; browsers require approved Origin. Handlers no longer read independent
legacy token gates. Persisted/config tuning and firmware tuning require additional
capabilities, while existing attestation/joint/rate/management guards remain.

54 gateway tests pass. Clippy passes after correctly restricting the fixture
builder to tests. Initial old positive command fixtures were authenticated,
without relaxing their original guard assertions. Compilation/collection
preparations are excluded from behavioral proof. See HTTP foundation receipts.

Still pending: all-route/capability conformance, WebTransport admission, runtime
UI/build-script migration, static marker build, required primary/Pi tests,
independent reviews and hosted CI. G07 stays open. No deployed or hardware
acceptance is claimed. All102 IDs/8 maintenance tasks and paused automation are
preserved. Movement requires per-test explicit owner confirmation and current
reference/support/E-stop/recovery qualification. Ten-minute silence allows only
independent work. Installed Pi4bc77ba/services remain unchanged.

## Local software checkpoint

The final targeted suite passes62 gateway tests, including29 actual protected
routes with four credential/Origin cases each,8 refused sensitive subscriptions,
25 role cases and2 body limits. Correct credentials retain independent existing
attestation, scope, rate and management assertions. Tuning checks show a Control
credential cannot persist/configure or request firmware tuning. HTTP and HTTPS
listeners are exercised with a trusted fixture certificate; actual pinned QUIC
checks public and sensitive delivery, Origin/capability refusal, oversized frames,
absent/partial subscription deadlines, released subscriptions and recovery.

Consul passes374 tests and builds. Runtime input reaches capability-specific
headers/subscriptions; entry/clear use no storage, old callbacks cannot restore
retired facts after credential changes, and connected follows typed admission.
Operator/scoped gateway entries and a separate Auto Learn field are available.
The mandatory runtime-connection topic remains in the admitted topic set.

An unchanged original production build materializes both supplied gateway and
Auto Learn markers in static assets and the actual marker oracle rejects it.
The qualified candidate build excludes all11 disposable markers from99 emitted
files. That check is now part of primary CI and native/Docker deployment builds.
No build reads/fetches Pi credentials into Vite. See frozen probes, original
marker receipt and local-software qualification. Fixture/collection corrections
are documented as preparation exclusions, not behavioral reds.

Remaining: primary gate, Pi native software fixtures, independent reviews and
exact-head PR/main CI/delivery. G07 remains open pending those gates. No physical
acceptance, deployment or motor permission is implied by these results.
