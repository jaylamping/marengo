# ADR 0028: capture chronology for imported log artifacts

**Status:** Accepted  
**Date:** 2026-10-01

## Context

ADR0011 stores capture starts as non-null epoch milliseconds. Parsing a literal
trailing `Z` as an offset-bearing datetime fails, so historic imports and archives
currently fabricate a start from the maintenance clock. Registration also invents
a capture end. Both distort browsing and age retention.

## Decision

Parse the complete canonical `YYYYMMDDTHHMMSSZ` capture ID as a Gregorian datetime
and explicitly assume UTC. The existing unsigned milliseconds contract supports
the Unix epoch, including zero; invalid or pre-epoch dates are not representable.
The actual producer's `profile-` prefix may precede one canonical capture ID.
Keep the original complete session ID as its identity.

Before importing any references or archiving any selected hot files, resolve every
selected session's start. Existing registered metadata is authoritative, including
arbitrary legacy IDs. A new capture without a recoverable date returns an explicit
error before row writes, gzip publication or hot-file removal. An operator can
register authoritative metadata explicitly and retry. This refusal policy avoids
inventing a timestamp or using epoch zero as an unknown sentinel.

Only regular bench/candump `.log` files and position-trace `.csv` files are captures.
Ignore exact `latest` aliases, symlinks, unsupported extensions and sidecars.
Archive valid capture IDs under their UTC date; registered IDs without a valid
capture date use the literal `unknown` bucket. Never slice arbitrary UTF-8 bytes
to construct dates.

Registration records an unknown end as SQL NULL. Only explicit finalization
supplies an end. Sparse updates and archive relocation preserve authoritative
starts and known ends. Existing stored rows lack provenance to distinguish earlier
fallback values from authoritative values; automatic retrospective repair is
deferred. Any future repair must back up, preview and preserve supplied metadata.

Expose retention's existing strict-before operation as `purge_before(cutoff_ms)`.
The days API computes its maintenance-clock cutoff and delegates. A supplied
cutoff must fit SQLite's signed milliseconds representation; refuse overflow
before deletion. Independent literal boundary cases can then qualify expiration
by capture start without relying on today's date or filesystem modification time.

## Consequences

Known-invalid metadata refusal is a preflight guarantee, not a transaction spanning
filesystem and SQL work. Later I/O/SQL errors and interrupted archive recovery
remain separate work, as do serialized migrations and bounded archive/page reads.
Capture chronology does not establish physical media durability, current-reference
permission or hardware acceptance. No robot operations, limits or Wave sign-off
changes follow from this decision.
