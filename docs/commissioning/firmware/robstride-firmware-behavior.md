# Robstride firmware 0.3.1.42: measured CAN behaviour

The right-arm bench has five Robstride drives on `can0` (device ids 1-5,
host id `0xFD`, MCP2515 / `mcp251x` on a Pi 5). Several of Davout's rules
exist because of how this firmware behaves on the wire. This page defines each
behaviour, gives the stats measured from bench captures, names the rule that
depends on it and that rule's margin over the measurement, and lists what
remains open.

- **Profile:** [`robstride-timing-profile.json`](robstride-timing-profile.json)
  (stats only, plus capture names and frame counts).
- **Analyzer:** `cargo run --release -p marengo-log-cli -- firmware-timing --json <candump>...`.
  It accepts `candump -L` and `candump -t z|a` lines. Its module doc
  (`bins/marengo-log-cli/src/firmware_timing.rs`) has the exact definitions.
- **Conformance:**
  [`crates/davout/tests/firmware_profile.rs`](../../../crates/davout/tests/firmware_profile.rs)
  loads the profile and fails if the firmware emulator
  (`crates/davout/tests/physical_firmware`) no longer covers a measured range,
  or if a Davout constant loses its margin.

To refresh the profile, scp new captures into `var/firmware-captures/`
(gitignored), rerun the analyzer on every non-empty capture, and rebuild the
profile. Then run `cargo test -p davout --test firmware_profile`.

## Captures

All captures are from 2026-10-03 (UTC). 394,887 frames over 371.7 s.

| Capture | Frames | Content |
|---|---:|---|
| `candump-20261003T011358Z.log` | 19,662 | streams and stop traffic only, no reference |
| `candump-20261003T141027Z.log` | 131 | 24 ms startup snippet |
| `candump-20261003T153408Z.log` | 28,453 | 5 references + enable (the identity-in-blackout incident) |
| `cd-20261003T024855Z.log` | 8,114 | 5 references + enable |
| `cd-20261003T031759Z.log` | 14,748 | 5 references + supervised hold |
| `cd-20261003T142407Z.log` | 23,283 | 5 references + enable |
| `cd-20261003T145133Z.log` | 56,933 | 5 references + supervised hold |
| `soak-20261003T163227Z.log` | 243,563 | 20-cycle no-motion enable soak at rev c879b6a: 100 references, 100 deferred Enables, zero overruns |

`cd-20261003T142319Z.log` is empty and was excluded. The first two use
`candump -t z`; the others use `-L`.

## How frames are read

**Definition.** `robstride::classify_frame` sorts each frame by where the host
id sits in the identifier:

- **Host command:** the host id sits above the target (`type << 24 | 0xFD << 8 | device`).
- **Drive frame:** the device sits above the host (`device << 8 | 0xFD`; for a type-0 reply, `device << 8 | 0xFE`).

A host frame's timestamp is its SocketCAN echo, which is when it reached the
wire. Every latency below is measured from that echo.

**Measured.** The mcp251x raises a frame's echo on TX-complete, so the drive's
answer can be read before the echo. In the soak, a status was read 70 µs and a
report 28 µs before the echo of the type-24 On they answered. The analyzer
allows 0.5 ms for this (`ECHO_LAG_S`). This lag is also why a few
`mit_reply_ms` samples fall below 0.1 ms: a status gets paired with the
following command's echo [INFERENCE].

## Enable → Run

**Definition.** `enable_to_run_ms`: time from the host Enable (type 3) to the
first type-2 or type-24 frame from that drive in Run mode (ID bits 22-23 = 2).
`reset_after_enable` counts Reset-mode frames from the drive after its Enable
and before that Run frame. `enable_never_run` counts Enables with no Run frame
within 1 s, or before the next Enable or Disable.

**Measured.** 245 Enables:

| Stat | Value |
|---|---|
| min | 1.36 ms |
| p50 per drive | 1.55-1.68 ms |
| p95 per drive | 4.60-5.02 ms |
| max | 10.30 ms |
| `reset_after_enable` | 0 |
| `enable_never_run` | 0 |

The 10.30 ms case (drive 3, `cd-20261003T031759Z`, before the Enable stagger)
had no reply to the Enable at all. The first Run frame was the SetZero
acknowledgement 10 ms later, consistent with a reply lost to an mcp251x RX
overrun. Without that case the max is 5.23 ms.

**Rule.** Davout arms the strict Run check at the target's own Enable echo
(`feedback_run_expected`, edb8fb3), never at the write. The check is safe
because every Run frame came after the echo (min 1.36 ms > 0), and no Reset
frame fell between an echo and its Run frame (0 of 245). That second fact
depends on the type-24 Off rule below. The emulator models
`enable_reply_delay` in [0, 11] ms.

**Open.** It is unknown whether a drive that receives an MIT frame inside the
1.4-5 ms window answers it in Reset. No capture shows one.

## Post-SetZero blackout

**Definition.** For each host SetZero (type 6), the analyzer finds the longest
gap between consecutive frames from that drive that meets all of these:

- The gap starts within 1.5 s of the SetZero.
- The gap is at least 20 ms long.
- The drive owed frames throughout the gap: its report stream was running, or
  host requests to it kept arriving with no 20 ms hole.

`set_zero_silence_start_ms` is the drive's last frame before the gap, measured
from the SetZero. The true start lies up to one report period later.
`set_zero_silence_ms` is the gap length.

**Measured.** 125 SetZeros produced 124 measurable blackouts. The 125th
(soak cycle 12, drive 1) fell where the host had stopped polling and the
stream was off, so no gap could be delimited.

| Stat | Value |
|---|---|
| start min | 511.4 ms |
| start p50 per drive | 516.9-523.0 ms |
| start p95 per drive | 538.6-543.4 ms |
| start max | 613.8 ms |
| length | 45.4-60.7 ms (p50 per drive 50.5-55.9 ms) |
| latest end after SetZero | 666.8 ms (next latest 599.5 ms) |

The drive receives nothing during the blackout:

- **Type-0:** 60 identity requests to drive 1 in the soak went unanswered
  inside its blackouts.
- **MIT:** 52-203 neutral MIT commands per drive went unanswered, all inside
  blackouts.
- **Enable:** an Enable written in the blackout leaves the drive in Reset
  (rev 15542aa incident).

**Trigger analysis.** Each of the 124 blackouts was compared against the
preceding host frames to the same drive, and against other drives' SetZeros:

| Anchor (time before silence start) | Range | sd | sd without drive 4 @ 15:34 |
|---|---|---:|---:|
| own SetZero | 511.4-613.8 ms | 11.8 | 8.6 |
| own 0x7019 read (+10 ms after SetZero) | 501.0-603.7 ms | 11.8 | 8.6 |
| own Enable (-10 ms before SetZero) | 521.8-623.9 ms | 11.8 | 8.6 |
| own identity request | 2.3-563.6 ms | 157 | 158 |
| own Disable / type-18 write | 15-524 ms | 157 | 156 |
| own type-24 Off | 0.5-644 ms | 213 | 212 |
| own type-24 On | 11-2798 ms | 1233 | 1235 |
| latest SetZero to another drive | 31-650 ms | 188 | — |

SetZero, the Enable before it and the 0x7019 read after it sit at fixed
offsets inside the reference sequence, so these captures cannot tell them
apart. The Enable is ruled out: 20 Enables sent more than 1 s after any
SetZero (the operator enables in the older captures) produced no gap longer
than 10.1 ms in the 0.3-0.9 s that followed. A 0x7019 read never occurs
without a SetZero in these captures. The other host events scatter by hundreds
of milliseconds, and another drive's SetZero does not line up either (only 1
of 124 silences starts 505-550 ms after another drive's SetZero). Duration
does not track start time (r = 0.13), but it does differ per drive: mean
49.1 ms for drive 3 against 55.9 ms for drive 4.

**The outlier.** In `candump-20261003T153408Z.log`, right_elbow_pitch (device
4) streamed type-24 reports without a gap through its usual 511-543 ms window.
It went silent 613.8 ms after its SetZero and came back at 666.8 ms with a
phase-shifted stream, meaning the stream restarted. Drives 1, 2 and 3 reported
normally throughout. No host frame reached drive 4 between its identity
request at +237.5 ms and the silence. Device 5 had its SetZero 92.9 ms after
device 4's, so drive 4's silence started 520.9 ms after device 5's SetZero,
which is inside the normal own-SetZero range. That is the only such alignment
in the data.

**Rule.** `POST_SET_ZERO_QUIET` is now 800 ms, up from 650 ms. That is the
latest measured blackout end (666.8 ms) plus 100 ms, rounded up to the next
50 ms. The conformance test checks the bound
`quiet ≥ max start + max length + 100 ms`. With start and length possibly from
different SetZeros, that is 774.5 ms, so the margin is 25.5 ms over the bound
and 133 ms over the actual latest end. On SocketCAN, no Enable and no gate Off
goes to a drive within the quiet after its SetZero echo (a2b55b3). The quiet
stays anchored on SetZero.

**Type-24 writes in the blackout.** The same drop applies to a type-24 On or
Off. Candump `decay-20261003T170858Z.log` (rev e6add09, three manual
`home` x5 runs, `-L` timestamps): in the first run pitch's SetZero is
40.029061; its baseline Offs for the next references are at +0.267, +0.364,
+0.456 and +0.554 s and the Ons after each commit at +0.319, +0.414, +0.507
and +0.605 s. Its blackout is the gap in its 10 ms stream from 40.562069
(+0.533) to 40.615508 (+0.586), 53.4 ms. The last Off (+0.554) fell inside it,
was dropped and the stream kept running, so that run passed; so did the second
(SetZero 43.34724, Off +0.542 inside the gap +0.516 to +0.569). A cadence 30-50
ms earlier puts the Off before the blackout and the On inside it, and the drive
then streams nothing until the host's 200 ms stale retry. The 14 failures of
the 20-cycle soak at the same revision are that case [INFERENCE: no candump
of a failing cycle exists; the soak runs did not capture one].
`POST_SET_ZERO_BLACKOUT_FROM` is 450 ms (earliest measured start 511.4 ms, less
a 50 ms margin held by `firmware_profile.rs`): from there to
`POST_SET_ZERO_QUIET` no type-24 On or Off is written to the drive.

**Cost.** The cost is 150 ms of enable latency. A target zeroed less than
800 ms earlier has its Enable held up to 150 ms longer than before. In a soak
cycle, the deferred enable completes about 800 ms after the last SetZero
instead of 650 ms. Targets zeroed earlier are not delayed.

**Emulator.** Each drive has its own `set_zero_blackout` (start, length). The
model allows start in [500, 625] ms and length in [40, 65] ms, so every
modeled blackout ends by 690 ms (`SET_ZERO_BLACKOUT`). The default is
(535, 55) ms. `enable_right_after_the_last_reference_is_held_past_the_set_zero_blackout`
puts ROLL at the latest modeled blackout (625-690 ms). With a 650 ms quiet,
ROLL's Enable lands inside that blackout and the test fails.

**Open.**

- What delayed drive 4's blackout by about 75 ms on 15:34? Possibly a
  firmware-internal flash write scheduled with device 5's SetZero, or a
  scheduling delay of its own [INFERENCE].
- Does the 0x7019 read, rather than SetZero, trigger the blackout? This needs a
  capture of a 0x7019 read without a SetZero.
- What sets the per-drive duration?

## Type-0 identity reply

**Definition.** `identity_reply_ms`: time from a host type-0 request to the
drive's reply (`device << 8 | 0xFE`, payload = MCU UID). A reply answers the
latest outstanding request. `identity_unanswered` counts requests that were
superseded, or got no reply within 100 ms.

**Measured.** 249 replies. min 0.13 ms, p50 0.15-0.16 ms, p95 0.17-0.29 ms,
max 0.47 ms. 61 requests went unanswered, all to drive 1: 60 were retries
during its blackouts in the soak, and 1 was the 15:34 incident. Every
admission was eventually answered.

**Rule.** `IDENTITY_ADMISSION_RETRY` is 10 ms and `IDENTITY_ADMISSION_TIMEOUT`
is 100 ms from the first request (169767a).

- **Retry:** 10 ms is more than 0.47 ms, so a normal reply is never chased by
  a retry.
- **Timeout:** the worst case is the longest blackout (60.7 ms), plus one
  retry period (10 ms), plus the slowest reply (0.47 ms), which is 71.2 ms.
  That leaves 28.8 ms of margin.

The emulator models `identity_reply_delay` in [0, 1] ms.

## Type-17 parameter read reply

**Definition.** `param_read_reply_ms`: time from a host type-17 read to the
drive's type-17 reply carrying the same index.

**Measured.** 125 replies (0x7019 `mechPos`), 0.12-0.30 ms, p50 0.17-0.18 ms.

**Rule.** The reference transaction's `AwaitReadback` phase has a 2 s
deadline, a margin of about 6,600× over the slowest reply.

## Type-24 active reporting

**Definition.**

- `report_period_ms`: interval between consecutive type-24 reports from a
  drive. Intervals that span a host type-24 write or a SetZero are excluded,
  as are intervals that start within 1.5 s after a SetZero.
- `report_off_to_last_ms`: time from a host type-24 Off, sent while the stream
  was running, to the last report after it. It is 0 when no report follows.
  A report read within 0.5 ms before the next type-24 write's echo counts as
  the answer to that write.

**Measured.**

| Stat | Value |
|---|---|
| report intervals | 109,871 |
| interval p50 | 10.00 ms |
| interval p95 | 10.03 ms |
| interval range | 7.42-12.59 ms (host timestamp jitter) |
| Offs measured | 392 |
| report after Off | 0 in all but 6, max 0.45 ms |

Drives keep a stream on across host processes: all five were streaming before
`marengo-pi` started at 14:51:33 and at 01:13:58.

**Rule.**

- **Off before every Enable (15542aa):** a target's Enable waits until its
  Off's echo was read at least one control period (5 ms at 200 Hz) earlier.
  The latest report after an Off is 0.45 ms, so the margin is 4.55 ms.
- **Paced writes:** type-24 writes are paced 5 ms per interface (6a1bb9d).

The emulator emits periodic reports at each drive's `report_period`, in
[7, 13] ms; the default is 10 ms.

**Open.** It is unknown whether a report already queued in the drive's TX
mailbox when the Off arrives can be sent later than 0.45 ms. None was
observed.

## Type-2 reply to every host frame

**Definition.** Every host frame of types 1, 3, 4, 6, 18 and 24 gets exactly
one type-2 status from the target. The analyzer pairs them FIFO per drive
inside a 5 ms window (one control period). `mit_reply_ms` is the MIT part of
that pairing. `mit_unanswered` counts MIT commands with no status within 5 ms.

**Measured.**

| Stat | Value |
|---|---|
| MIT replies | 133,815 |
| p50 | 0.18-0.22 ms |
| p95 | 0.20-0.25 ms |
| max | 4.96 ms |
| unanswered MIT, drive 1 | 52 |
| unanswered MIT, drives 2-5 | 175-203 each |

The unanswered MIT fall inside post-SetZero blackouts while the soak loop sent
neutral MIT at 200 Hz.

**Rule.** Because every host frame produces a reply, Enable and type-24 writes
are staggered: one target per interface per tick, and type-24 writes paced
5 ms per interface (6a1bb9d). The mcp251x keeps only two RX buffers. Bus
density peaked at 69 frames in any 10 ms window. The soak had zero RX
overruns.

**Stop burst overrun.** Type 18 (`spd_ref` zero) is also answered, so the
all-address stop (type 18, MIT, Disable per address) is 15 frames and 15
replies (3.3 received frames/ms over 4.5 ms). Candump `decay-...Z.log`, third
run (delay 3 s): the reference's finishing stop at 47.8910-47.8955 went out
back to back and the next drain read a `CAN_ERR_CRTL_RX_OVERFLOW` frame
(`rx_over_errors` 5 to 6, Transport latched, the reference failed
`Invalidated(SafetyHazard)` at 17:09:07.901). The baseline stop of that run
(47.8385-47.8432, the same 15 frames) did not overrun. Davout now starts one
address group per 2 ms per interface in the reference's baseline and finishing
stops (1.5 received frames/ms), and the same for the baseline's type-24 Offs,
the Enable-admission type-0 requests and the status solicit. The emulator
models the receive path (`RxFifo`: bus slot per frame, two buffers, 350 us
driver service per frame); calibrated on these observations, it overruns from
the ninth reply of an unpaced 15-frame stop and not on five-drive MIT batches
or the paced soak traffic.

## Disable → Reset

**Definition.** `disable_to_reset_ms`: time from a host Disable (type 4) to
the first Reset-mode status or report from that drive within 100 ms. An
Enable cancels the wait.

**Measured.** 1,556 Disables:

| Stat | Value |
|---|---|
| min | 0.14 ms |
| p50 | 0.16-0.17 ms |
| p95 | 4.2-4.7 ms |
| max | 48.7 ms |

A Disable sent while the drive is in Run is often answered late: 0.4-23 ms in
the soak. Two silences of 45-48 ms followed a Disable plus type-24 Off with no
SetZero involved (`cd-20261003T031759Z` drive 3, soak drive 2).

**Rule.** No Davout rule waits on this reply. The stop path writes Disable to
every address without awaiting it. The 48.7 ms worst case is below the 100 ms
`comm_watchdog_ms` liveness bound.

**Open.** It is unknown whether the post-Disable silence from Run is the same
mechanism as the post-SetZero blackout. It is shorter, but similar in length.

## Wire-level MIT neutrality

**Definition.** `non_neutral_mit` counts MIT command frames (type 1) whose raw
kp or kd code is nonzero, or whose torque-feedforward code differs from
neutral `0x7FFF` by more than one quantization step.

**Measured.**

- The soak sent 0 non-neutral frames, which is what `pi_enable_soak` requires.
- `cd-20261003T031759Z` (2,480) and `cd-20261003T145133Z` (21,750) were
  supervised hold sessions, where non-neutral commands are expected.

**Rule.** `pi_enable_soak` fails unless the analyzer reports 0.
