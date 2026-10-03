/**
 * Gravity-model gate run before any enable by pi_hold_on and pi_bench_harness
 * (limb-playbook §4a/4b: per-joint |τ_meas − τ_g| < 0.20 Nm).
 *
 * τ_g comes from `motor-repl gravity-preview` (read-only model evaluation), which still
 * opens SocketCAN, so the caller runs the preview shell only as the sole CAN owner.
 *
 * Measured torque comes from the gateway RobotState snapshot (never CAN). It is used
 * only when the snapshot is fresh and every gated joint is drive-active and Verified,
 * i.e. marengo-pi is holding the arm. A disabled drive carries no phase current and
 * reports ~0 Nm whatever the load, which is not a gravity measurement; then the gate
 * evaluates τ_g at the profile's hanging rest pose instead, where the physical gravity
 * torque is ~0, and refuses when |τ_g| ≥ 0.20 Nm.
 */

import { type BenchProfile, profileMeta } from "./bench-profiles.js";
import {
  type RobotStateSnapshot,
  decodeRobotState,
  robotStateMarkerPayload,
  robotStateSnapshotShell,
} from "./robot-state.js";

/** Per-joint residual bar (limb-playbook §4b), also applied to |τ_g| at the hanging rest. */
export const GRAVITY_RESIDUAL_LIMIT_NM = 0.2;

/** Gateway snapshot age (Pi clock) up to which its drive torque counts as current. */
export const SNAPSHOT_FRESH_MS = 1000;

const PI_NOW_MARKER = "pi_now_ms=";
const JOINT_NAME = /^[A-Za-z0-9_]+$/;
const TAU_G_LINE = /^([A-Za-z0-9_]+): tau_g = (-?\d+(?:\.\d+)?) Nm$/gm;

type GateBasis = "measured" | "hanging_rest";

interface GravityGatePlan {
  basis: GateBasis;
  joints: readonly string[];
  /** Preview pose by joint name; model joints not listed are evaluated at 0. */
  pose: Record<string, number>;
  /** Drive-reported joint torque per gated joint (measured basis only). */
  measuredNm: Record<string, number>;
  /** Why this basis applies. */
  note: string;
}

export interface GravityGateResult {
  ok: boolean;
  /** Human-readable verdict; a refusal starts its last line with `FAIL gravity_model_mismatch` or `FAIL gravity_gate_unavailable`. */
  report: string;
}

/** Remote shell, never opens CAN: Pi wall clock plus the gateway RobotState snapshot. */
export function gravityGateSnapshotShell(): string {
  return [`printf '${PI_NOW_MARKER}%s\\n' "$(date +%s%3N)"`, robotStateSnapshotShell()].join("\n");
}

/** The decoded snapshot when it supplies current drive torque for every joint, else why not. */
function liveDriveSnapshot(
  snapshotOutput: string,
  joints: readonly string[],
): { reason: string } | { state: RobotStateSnapshot } {
  const b64 = robotStateMarkerPayload(snapshotOutput);
  if (!b64) return { reason: "gateway RobotState snapshot unavailable" };
  let state: RobotStateSnapshot;
  try {
    state = decodeRobotState(Buffer.from(b64, "base64"));
  } catch (err) {
    return { reason: `gateway RobotState undecodable (${err instanceof Error ? err.message : String(err)})` };
  }
  const nowMs = Number(snapshotOutput.match(new RegExp(`^${PI_NOW_MARKER}(\\d+)$`, "m"))?.[1]);
  const ageMs = nowMs - state.timestampMs;
  if (!Number.isFinite(ageMs) || Math.abs(ageMs) > SNAPSHOT_FRESH_MS) {
    return { reason: `gateway RobotState not current (age ${Number.isFinite(ageMs) ? `${ageMs} ms` : "unknown"})` };
  }
  const idle = joints.filter((j) => {
    const s = state.joints.find((x) => x.name === j);
    return !s || !s.driveActive || s.homing !== "Verified";
  });
  if (idle.length > 0) {
    return { reason: `not drive-active+Verified: ${idle.join(", ")}` };
  }
  return { state };
}

function planGravityGate(
  profile: BenchProfile,
  joints: readonly string[],
  snapshotOutput: string,
): GravityGatePlan {
  const live = liveDriveSnapshot(snapshotOutput, joints);
  if ("reason" in live) {
    return {
      basis: "hanging_rest",
      joints,
      pose: { ...profileMeta(profile).hangingRestRad },
      measuredNm: {},
      note:
        `${live.reason}; a disabled drive reports ~0 Nm regardless of load, so the gate checks |τ_g| ` +
        `at the ${profile} hanging rest pose (physical gravity torque ~0)`,
    };
  }
  const named = live.state.joints.filter((s) => JOINT_NAME.test(s.name));
  return {
    basis: "measured",
    joints,
    pose: Object.fromEntries(named.map((s) => [s.name, s.position])),
    measuredNm: Object.fromEntries(
      named.filter((s) => joints.includes(s.name)).map((s) => [s.name, s.effort]),
    ),
    note: "marengo-pi holds every gated joint; comparing gateway drive torque with τ_g at the published pose",
  };
}

/**
 * Remote shell printing `<joint>: tau_g = <Nm> Nm` at `pose`. The caller must be the sole
 * CAN owner. gravity-preview takes a full robot.yaml-order vector (a partial one silently
 * becomes all zeros), so a non-zero pose first reads the model's joint order.
 */
function gravityPreviewShell(pose: Record<string, number>): string {
  const nonzero = Object.entries(pose).filter(
    ([j, q]) => JOINT_NAME.test(j) && Number.isFinite(q) && q !== 0,
  );
  if (nonzero.length === 0) return "bin/motor-repl gravity-preview";
  return [
    `GG_NAMES="$(bin/motor-repl gravity-preview | sed -n 's/^\\([A-Za-z0-9_]*\\): tau_g = .*/\\1/p')"`,
    'test -n "$GG_NAMES" || { echo "gravity gate: gravity-preview printed no joints" >&2; exit 1; }',
    'GG_Q=""',
    "for GG_J in $GG_NAMES; do",
    '  case "$GG_J" in',
    ...nonzero.map(([j, q]) => `    ${j}) GG_Q="$GG_Q ${q}" ;;`),
    '    *) GG_Q="$GG_Q 0" ;;',
    "  esac",
    "done",
    "bin/motor-repl gravity-preview $GG_Q",
  ].join("\n");
}

/** Gate verdict plus whether the only failure was a residual mismatch (not unavailability). */
function evaluateGravityGate(
  plan: GravityGatePlan,
  previewOutput: string,
): GravityGateResult & { mismatchOnly: boolean } {
  const tauG = new Map(
    [...previewOutput.matchAll(TAU_G_LINE)].map((m) => [m[1], Number(m[2])] as const),
  );
  const header = `gravity gate (basis=${plan.basis}): ${plan.note}`;
  const lines: string[] = [];
  const mismatched: string[] = [];
  for (const j of plan.joints) {
    const tau = tauG.get(j);
    const q = (plan.pose[j] ?? 0).toFixed(4);
    if (tau === undefined) {
      lines.push(`  ${j}: q=${q} rad — not in the gravity model (no τ_g feed-forward); not gated`);
      continue;
    }
    const meas = plan.measuredNm[j];
    const residual = Math.abs((meas ?? 0) - tau);
    const bad = residual >= GRAVITY_RESIDUAL_LIMIT_NM;
    if (bad) mismatched.push(j);
    const measured = meas === undefined ? "" : ` τ_meas=${meas.toFixed(4)} Nm`;
    lines.push(
      `  ${j}: q=${q} rad τ_g=${tau.toFixed(4)} Nm${measured} residual=${residual.toFixed(4)} Nm ${bad ? "FAIL" : "ok"}`,
    );
  }
  const limit = GRAVITY_RESIDUAL_LIMIT_NM.toFixed(2);
  if (!plan.joints.some((j) => tauG.has(j))) {
    return {
      ok: false,
      mismatchOnly: false,
      report: [
        header,
        ...lines,
        `FAIL gravity_gate_unavailable: gravity-preview returned no τ_g for ${plan.joints.join(", ")}; refusing before any motion`,
      ].join("\n"),
    };
  }
  const residualKind = plan.basis === "measured" ? "|τ_meas − τ_g|" : "|τ_g| at the hanging rest";
  const verdict =
    mismatched.length > 0
      ? `FAIL gravity_model_mismatch: ${residualKind} ≥ ${limit} Nm on ${mismatched.join(", ")}. ` +
        "The URDF gravity model disagrees with the physical arm; refusing before any motion. " +
        "Fix the model (limb-playbook §4a/4b) — do not raise gains to overpower it."
      : `PASS gravity gate: ${residualKind} < ${limit} Nm on every modeled joint`;
  return {
    ok: mismatched.length === 0,
    mismatchOnly: mismatched.length > 0,
    report: [header, ...lines, verdict].join("\n"),
  };
}

/** Last report line when {@link runGravityGate} lets a hanging-rest mismatch through. */
export const HANGING_REST_MISMATCH_SKIPPED =
  "SKIPPED gravity_model_mismatch (hanging rest) for gravity calibration: residuals reported, not gating";

/**
 * The gate. `snapshotOutput` is remote output of {@link gravityGateSnapshotShell}, captured
 * before the caller stopped marengo-pi; `runPreview` runs a shell as the sole CAN owner.
 * `allowHangingRestMismatch` (gravity calibration only, which exists to fix the model) passes a
 * hanging-rest |τ_g| mismatch with the full residual report; `gravity_gate_unavailable` and a
 * measured-basis mismatch still refuse.
 */
export async function runGravityGate(opts: {
  profile: BenchProfile;
  joints: readonly string[];
  snapshotOutput: string;
  runPreview: (shell: string) => Promise<string>;
  allowHangingRestMismatch?: boolean;
}): Promise<GravityGateResult> {
  const plan = planGravityGate(opts.profile, opts.joints, opts.snapshotOutput);
  const previewOutput = await opts.runPreview(gravityPreviewShell(plan.pose));
  const { ok, mismatchOnly, report } = evaluateGravityGate(plan, previewOutput);
  if (ok) return { ok, report };
  const withPreview = `${previewOutput.trimEnd()}\n${report}`;
  if (opts.allowHangingRestMismatch === true && plan.basis === "hanging_rest" && mismatchOnly) {
    return { ok: true, report: `${withPreview}\n${HANGING_REST_MISMATCH_SKIPPED}` };
  }
  return { ok: false, report: withPreview };
}
