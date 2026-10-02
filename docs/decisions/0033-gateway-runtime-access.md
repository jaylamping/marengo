# ADR 0033: shared gateway access policy and runtime credentials

Status: software qualified at dc88d28; final PR/main delivery checks pending, October 2, 2026.

G07 identifies legacy command and sensitive-stream paths outside the configured
gateway credential gate. Credentials embedded by Vite builds are also delivered
to browsers as static assets. The personal robot's project access policy must
cover these paths without granting motor permission or replacing Davout.

Resolve trusted role credentials and allowed browser origins once when composing
gateway state. Keep one immutable policy for HTTP, HTTPS and WebTransport.
Capabilities distinguish control, calibration, configuration, management and
sensitive logs/audit streams. Preserve the existing configured administrative
credential as a documented compatibility role; offer narrower role credentials.
Unset or invalid credentials fail closed. Compare credentials without logging
them. Public health and ordinary telemetry remain available without credentials.

Apply authorization before body parsing, command publication, file changes,
management work or sensitive subscription. Require explicit allowed browser
origins while allowing credential-bearing non-browser clients without Origin.
Retain confirmation, attestation, rate, freshness, runtime owner and Davout gates
after access admission. Authorizing a gateway request is not motor authority.

Extend the protobuf WebTransport subscription with a runtime credential and a
bounded admission response. Bound handshake time and size before subscribing;
reject a mixed public/sensitive subscription without its required capability.
Consul should subscribe to public topics before runtime credential entry and
reconnect deliberately when that credential changes. Mark transport connected
only after subscription admission.

Consul obtains gateway and Auto Learn credentials through runtime entry and
retains them only in tab memory. Shared API helpers read that state. Build and
deployment scripts must stop reading/copying credentials into VITE_* values;
endpoint URLs remain ordinary configuration. This work must be tested with known
isolated markers in build inputs and actual request headers/subscriptions.

Acceptance requires original-public refusal probes, authenticated and capability
controls, independent existing attestation/rate checks, HTTP/HTTPS and actual
QUIC subscription tests, required CI and native Pi software tests. All fixtures
use an isolated Bus, disposable files and loopback listeners with no installed
runtime, CAN or motors. Deployment and physical commissioning remain separate.

Implementation uses five trusted capability bits. OPERATOR and the legacy LOG
credential carry all five; CONTROL, CALIBRATION, CONFIG, MANAGEMENT and READ
startup credentials carry their corresponding bit. Reusing one trusted value
in multiple slots unions those capabilities. HTTP accepts Bearer or the legacy
header, refusing ambiguous/duplicated credentials. Stored/config tuning needs
Control plus Configuration; firmware tuning needs Control plus Calibration.

Exact local Vite origins and additional configured origins are permitted.
Robot-hosted HTTPS uses the trusted HTTPS listener's scheme/port and the target
request authority host, including QUIC's separate UDP authority. Non-browser
requests can omit Origin. WebTransport restricts the CONNECT path/query and
requires its sensitive credential in the first protobuf subscription. Combined
stream acceptance/frame reading has a five-second deadline and16KiB preallocation
limit; admission writes have a one-second bound.64 owned session tasks and bounded
refusal work limit application sessions. Underlying HTTP/3 negotiation belongs to
the transport library and is not qualified as a separate connection-queue repair.
HTTP extraction retains Axum's2MiB body limit. Mandatory runtime-connection
invalidation is included in the admitted topic set before envelopes.

Consul selects runtime credentials by capability with an explicit operator
fallback. Read credential changes reconnect and retire old callbacks/facts.
No credentials use browser storage. The qualified build injects11 disposable
credential markers into a child build and scans all emitted files. Native and
Docker deployment paths stop reading/fetching Pi credentials into Vite.
