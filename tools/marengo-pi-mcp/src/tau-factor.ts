/**
 * Per-joint model-uncertainty factor of the τ guard (pi_joint_calibrate, pi_motion_suite):
 * the guard requires factor × |τ_g| ≤ 0.8 × τ_ff cap.
 *
 * A joint whose gravity was fitted and applied gets factor 1 + max(3σ_A/A, 0.15), from its
 * accepted gravity-fit record (docs/commissioning/calibrations/<record>.json), when all of:
 * - the calibrations index (`applied-gravity.json`) lists the joint with that record and the
 *   gravity-model fingerprint of the URDF the fit was applied to;
 * - the Pi URDF has that fingerprint (else the entry is stale: the URDF changed since);
 * - the Pi URDF carries the record's fitted link masses and COMs (the fit was applied);
 * - the configuration holds every other joint at the fit's fixed pose (others at 0 unless the
 *   record says otherwise), the only configurations the fit measured.
 * Anything else, including a missing or unreadable index or record, fails closed to the
 * unverified factor 1.6.
 */

import { createHash } from "node:crypto";
import path from "node:path";
import { z } from "zod";

/** Factor on |τ_g| without fitted evidence (elbow measured ≈ 1.5 × URDF on 2026-10-04). */
export const UNVERIFIED_TAU_FACTOR = 1.6;
/** Smallest model-error share a calibrated joint is credited with. */
export const CALIBRATED_TAU_MARGIN = 0.15;
/** σ_A multiple the calibrated factor covers. */
export const CALIBRATED_SIGMAS = 3;
/** Calibrations index, relative to the repo root. */
export const GRAVITY_INDEX_PATH = "docs/commissioning/calibrations/applied-gravity.json";
export const GRAVITY_INDEX_VERSION = 1;
/** How far another joint may sit from the fit's fixed pose and keep the calibrated factor (rad). */
const FIXED_POSE_TOL_RAD = 1e-3;
/** Records and the URDF print COMs to 0.01 mm and masses to 0.1 g. */
const COM_TOL_M = 1.5e-5;
const MASS_TOL_KG = 1.5e-4;
const RECORD_NAME = /^[A-Za-z0-9._-]+\.json$/;

export interface TauFactors {
  /** Factor on `joint`'s |τ_g| at `config` (joints not listed are at 0). */
  at(joint: string, config: Readonly<Record<string, number>>): number;
  /** Header line plus one line per joint: factor and its basis. */
  report: string[];
}

interface Inertial {
  mass?: number;
  xyz: number[];
}

const stripComments = (xml: string) => xml.replace(/<!--[\s\S]*?-->/g, "");

/** `<name …>body</name>` or `<name …/>` elements: opening-tag attributes and body. */
function elements(xml: string, name: string): { attrs: string; body: string }[] {
  const re = new RegExp(`<${name}\\b([^>]*?)(?:/>|>([\\s\\S]*?)</${name}>)`, "g");
  return [...xml.matchAll(re)].map((m) => ({ attrs: m[1], body: m[2] ?? "" }));
}

function attr(attrs: string | undefined, name: string): string | undefined {
  return attrs === undefined ? undefined : new RegExp(`\\b${name}="([^"]*)"`).exec(attrs)?.[1];
}

/** Attributes of the first `<name …>` tag in `body`. */
function firstTag(body: string, name: string): string | undefined {
  return new RegExp(`<${name}\\b([^>]*?)/?>`).exec(body)?.[1];
}

/** Whitespace-separated numbers in canonical form (so 0.0 and 0 agree); non-numbers verbatim. */
function canonicalNumbers(value: string | undefined, fallback: string): string {
  return (value ?? fallback)
    .trim()
    .split(/\s+/)
    .map((v) => (Number.isFinite(Number(v)) ? String(Number(v)) : v))
    .join(" ");
}

function linkInertials(urdf: string): Map<string, Inertial> {
  const out = new Map<string, Inertial>();
  for (const link of elements(stripComments(urdf), "link")) {
    const name = attr(link.attrs, "name");
    const inertial = elements(link.body, "inertial")[0];
    if (name === undefined || inertial === undefined) continue;
    const mass = Number(attr(firstTag(inertial.body, "mass"), "value"));
    out.set(name, {
      mass: Number.isFinite(mass) ? mass : undefined,
      xyz: canonicalNumbers(attr(firstTag(inertial.body, "origin"), "xyz"), "0 0 0").split(" ").map(Number),
    });
  }
  return out;
}

/**
 * sha256 over what the gravity model reads from a URDF: every link's inertial mass and COM
 * origin, every joint's type, parent, child, origin and axis. Limits (Set Limits writes them,
 * ADR 0017), inertia tensors, visuals, comments and formatting do not count.
 */
export function gravityModelFingerprint(urdf: string): string {
  const xml = stripComments(urdf);
  const lines: string[] = [];
  for (const link of elements(xml, "link")) {
    const inertial = elements(link.body, "inertial")[0];
    if (inertial === undefined) {
      lines.push(`link ${attr(link.attrs, "name")} -`);
      continue;
    }
    const origin = firstTag(inertial.body, "origin");
    lines.push(
      `link ${attr(link.attrs, "name")} mass=${canonicalNumbers(attr(firstTag(inertial.body, "mass"), "value"), "-")} ` +
        `xyz=${canonicalNumbers(attr(origin, "xyz"), "0 0 0")} rpy=${canonicalNumbers(attr(origin, "rpy"), "0 0 0")}`,
    );
  }
  for (const joint of elements(xml, "joint")) {
    const type = attr(joint.attrs, "type");
    if (type === undefined) continue;
    const origin = firstTag(joint.body, "origin");
    lines.push(
      `joint ${attr(joint.attrs, "name")} ${type} parent=${attr(firstTag(joint.body, "parent"), "link")} ` +
        `child=${attr(firstTag(joint.body, "child"), "link")} xyz=${canonicalNumbers(attr(origin, "xyz"), "0 0 0")} ` +
        `rpy=${canonicalNumbers(attr(origin, "rpy"), "0 0 0")} axis=${canonicalNumbers(attr(firstTag(joint.body, "axis"), "xyz"), "1 0 0")}`,
    );
  }
  return createHash("sha256").update(lines.join("\n")).digest("hex");
}

type Evidence = { ok: true; factor: number; fixedRad: Record<string, number>; basis: string } | { ok: false; reason: string };

const indexSchema = z.object({ version: z.literal(GRAVITY_INDEX_VERSION), joints: z.record(z.unknown()) });
const entrySchema = z.object({
  record: z.string().regex(RECORD_NAME, "record name must be a .json file in the calibrations directory"),
  urdf_gravity_sha256: z.string(),
});
const groupSchema = z.object({
  joint: z.string(),
  accepted: z.boolean(),
  fixed_rad: z.record(z.number()).default({}),
  fit: z.object({ a_nm: z.number(), sigma_a_nm: z.number().nonnegative() }).nullish(),
});
const recordSchema = z.object({
  kind: z.literal("gravity_calibration"),
  accepted: z.literal(true),
  groups: z.array(z.unknown()),
  urdf_patch: z
    .object({
      links: z
        .array(
          z.object({
            link: z.string(),
            mass_kg: z.number(),
            com_m: z.object({ fitted: z.tuple([z.number(), z.number(), z.number()]) }),
          }),
        )
        .min(1),
    })
    .nullish(),
});

/** First zod issue as `path: message`. */
function issue(err: z.ZodError): string {
  const first = err.issues[0];
  return first === undefined ? err.message : `${first.path.join(".") || "root"}: ${first.message}`;
}

function readError(what: string, err: unknown): string {
  const missing = typeof err === "object" && err !== null && "code" in err && err.code === "ENOENT";
  return missing ? `${what} missing` : `${what} unreadable: ${String(err)}`;
}

/** The fitted-and-applied evidence for `joint` from its index entry, or why there is none. */
async function jointEvidence(
  joint: string,
  rawEntry: unknown,
  fingerprint: string,
  inertials: ReadonlyMap<string, Inertial>,
  readLocal: (rel: string) => Promise<string>,
): Promise<Evidence> {
  if (rawEntry === undefined) return { ok: false, reason: "not in the index" };
  const entry = entrySchema.safeParse(rawEntry);
  if (!entry.success) return { ok: false, reason: `index entry: ${issue(entry.error)}` };
  const { record: name, urdf_gravity_sha256: pinned } = entry.data;
  if (pinned !== fingerprint) {
    return { ok: false, reason: `stale: the Pi URDF gravity model changed since the fit was applied (index ${pinned.slice(0, 12)})` };
  }
  let raw: unknown;
  try {
    raw = JSON.parse(await readLocal(path.posix.join(path.posix.dirname(GRAVITY_INDEX_PATH), name)));
  } catch (err) {
    return { ok: false, reason: readError(`record ${name}`, err) };
  }
  const record = recordSchema.safeParse(raw);
  if (!record.success) return { ok: false, reason: `record ${name}: ${issue(record.error)}` };
  const group = record.data.groups
    .map((g) => groupSchema.safeParse(g))
    .flatMap((g) => (g.success ? [g.data] : []))
    .find((g) => g.joint === joint);
  if (group === undefined || !group.accepted || group.fit == null || group.fit.a_nm === 0) {
    return { ok: false, reason: `record ${name} has no accepted fit with A ≠ 0 and σ_A ≥ 0 for ${joint}` };
  }
  const links = record.data.urdf_patch?.links;
  if (links === undefined) return { ok: false, reason: `record ${name} carries no fitted link COMs to check against the URDF` };
  for (const link of links) {
    const urdf = inertials.get(link.link);
    const applied =
      urdf?.mass !== undefined &&
      Math.abs(urdf.mass - link.mass_kg) <= MASS_TOL_KG &&
      link.com_m.fitted.every((c, i) => Math.abs(urdf.xyz[i] - c) <= COM_TOL_M);
    if (!applied) return { ok: false, reason: `fit not applied: ${link.link} mass/COM in the Pi URDF differ from record ${name}` };
  }
  const { a_nm: a, sigma_a_nm: sigma } = group.fit;
  const fixedRad = group.fixed_rad;
  const share = (CALIBRATED_SIGMAS * sigma) / Math.abs(a);
  // To 0.001, upward.
  const factor = Math.ceil((1 + Math.max(share, CALIBRATED_TAU_MARGIN)) * 1000 - 1e-9) / 1000;
  const fixed = Object.entries(fixedRad).filter(([, q]) => q !== 0);
  return {
    ok: true,
    factor,
    fixedRad,
    basis:
      `${name}: A ${a} ± ${sigma} Nm, ${CALIBRATED_SIGMAS}σ/A ${(100 * share).toFixed(1)} % (floor ` +
      `${100 * CALIBRATED_TAU_MARGIN} %); other joints at ${fixed.length === 0 ? "0" : fixed.map(([j, q]) => `${j}=${q}`).join(" ")}`,
  };
}

/**
 * Factors for `joints` from the repo calibrations index (`readLocal`: repo-relative path →
 * content) against the URDF the guard's τ_g comes from. Never refuses: missing, unreadable or
 * stale evidence leaves the joint at {@link UNVERIFIED_TAU_FACTOR}, with the reason reported.
 */
export async function resolveTauFactors(input: {
  urdf: string;
  joints: readonly string[];
  readLocal: (rel: string) => Promise<string>;
}): Promise<TauFactors> {
  const fingerprint = gravityModelFingerprint(input.urdf);
  let entries: Record<string, unknown> | undefined;
  let indexIssue: string | undefined;
  try {
    const index = indexSchema.safeParse(JSON.parse(await input.readLocal(GRAVITY_INDEX_PATH)));
    if (index.success) entries = index.data.joints;
    else indexIssue = `index unreadable: ${issue(index.error)}`;
  } catch (err) {
    indexIssue = readError("index", err);
  }
  const calibrated = new Map<string, { factor: number; fixedRad: Record<string, number> }>();
  const report = [`τ guard factors (${GRAVITY_INDEX_PATH}; Pi URDF gravity model sha256 ${fingerprint.slice(0, 12)}):`];
  const inertials = linkInertials(input.urdf);
  for (const joint of input.joints) {
    const evidence: Evidence =
      entries === undefined
        ? { ok: false, reason: indexIssue ?? "index unreadable" }
        : await jointEvidence(joint, entries[joint], fingerprint, inertials, input.readLocal);
    if (evidence.ok) {
      calibrated.set(joint, evidence);
      report.push(`  ${joint}: × ${evidence.factor} calibrated (${evidence.basis})`);
    } else {
      report.push(`  ${joint}: × ${UNVERIFIED_TAU_FACTOR} unverified (${evidence.reason})`);
    }
  }
  return {
    at(joint, config) {
      const c = calibrated.get(joint);
      if (c === undefined) return UNVERIFIED_TAU_FACTOR;
      for (const other of new Set([...Object.keys(config), ...Object.keys(c.fixedRad)])) {
        if (other !== joint && Math.abs((config[other] ?? 0) - (c.fixedRad[other] ?? 0)) > FIXED_POSE_TOL_RAD) {
          return UNVERIFIED_TAU_FACTOR;
        }
      }
      return c.factor;
    },
    report,
  };
}
