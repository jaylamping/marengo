# hardware-management-api Specification

## Purpose

Authenticated gateway APIs for master URDF lifecycle, import field resolution, archive/restore, and ephemeral limb subsets. All durable mutations MUST target master paths only; staging and archive are transitional, not parallel SoTs.

## Requirements

### Requirement: Authenticated URDF read

The gateway MUST expose an authenticated read of the active `marengo.urdf` bytes and metadata (checksum, revision). Unauthenticated or invalid-token requests MUST fail without leaking file contents.

#### Scenario: Authorized read

- GIVEN a valid gateway auth token
- WHEN the client requests active URDF
- THEN response includes URDF body or signed fetch URL and checksum metadata

#### Scenario: Unauthorized read rejected

- GIVEN no or invalid auth
- WHEN the client requests active URDF
- THEN response is unauthorized and no URDF bytes are returned

### Requirement: URDF upload and staging

Upload MUST write to on-disk `staging/` under the URDF assets root. Each upload MUST receive a unique `upload-id`. Staging MUST NOT replace `marengo.urdf` until an explicit activate succeeds.

#### Scenario: Upload lands in staging

- GIVEN a valid URDF file and auth
- WHEN the client uploads URDF v1
- THEN bytes are stored under `staging/` keyed by `upload-id` and active `marengo.urdf` is unchanged

### Requirement: Field resolution before activation

After upload, the API MUST expose per-field resolution between staging contributor and current active model. Kinematics-critical fields (joint origin, axis, parent link, limit hard envelope) MUST require explicit operator choice before activate. Non-critical conflicts MAY default with override logged in the resolution payload.

#### Scenario: Critical kinematics conflict blocks silent activate

- GIVEN staging and active URDF disagree on a joint axis
- WHEN the client requests activate without a resolution pick for that field
- THEN activate is rejected with the unresolved field identified

#### Scenario: Resolved activate applies choices

- GIVEN the operator selected per-field winners for all required conflicts
- WHEN activate succeeds
- THEN merged result becomes the candidate active model per resolved fields only

### Requirement: Activate archives prior contributor

Successful activate MUST atomically promote the merged model to `marengo.urdf` and archive the replaced active bytes under `archive/<upload-id>/` with `manifest.json` per research #108 (checksum, timestamp, source upload-id, operator metadata).

#### Scenario: Activate promotes and archives

- GIVEN resolved staging ready to activate
- WHEN activate completes
- THEN `marengo.urdf` reflects the merged model AND prior active bytes exist under `archive/<upload-id>/manifest.json`

### Requirement: Archive list, fetch, and restore

The gateway MUST list archive entries, fetch archive URDF by `upload-id`, and restore a chosen archive entry to active via the same activate safety rules (resolution if merge conflicts exist). Restore MUST NOT leave mixed old/new writers on disk.

#### Scenario: List and fetch archive

- GIVEN at least one archived upload
- WHEN the client lists then fetches by `upload-id`
- THEN manifest metadata and archived URDF bytes are returned

#### Scenario: Restore replaces active safely

- GIVEN a valid archive `upload-id`
- WHEN restore succeeds
- THEN `marengo.urdf` matches restored content and the replaced active file is archived

### Requirement: Completeness API

The gateway MUST expose completeness v1 status derived with `hardware-description-sot` rules. Responses MUST be warning lists with stable codes; they MUST NOT include blocking flags that prevent other API operations.

#### Scenario: Completeness is advisory

- GIVEN completeness returns warnings
- WHEN the client calls upload, activate, or limit patch endpoints
- THEN those calls are not rejected solely for completeness warnings

### Requirement: Ephemeral subset contract

Subset endpoints MUST return filtered views of master config/URDF in memory or transient response bodies. They MUST NOT write subset-specific durable URDF or YAML paths.

#### Scenario: Subset response is ephemeral

- GIVEN a request for a limb subset (e.g., right 4-DOF arm)
- WHEN the subset endpoint responds
- THEN the response filters master data and no new durable SoT path is created

### Requirement: Profile route retirement

Gateway `/config/profiles*` routes and profile registry transactions for runtime SoT MUST be removed or return gone. Inactive profile CAS apply paths MUST NOT remain the operator path for durable hardware edits.

#### Scenario: Profiles endpoint retired

- GIVEN cutover is complete
- WHEN the client calls `GET /config/profiles`
- THEN the route is not served as the hardware description registry
