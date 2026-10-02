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
