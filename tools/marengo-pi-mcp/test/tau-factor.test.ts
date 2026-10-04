import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { MASTER_JOINTS } from "../src/bench-profiles.js";
import {
  GRAVITY_INDEX_PATH,
  UNVERIFIED_TAU_FACTOR,
  gravityModelFingerprint,
  resolveTauFactors,
} from "../src/tau-factor.js";

const REPO = new URL("../../../", import.meta.url);
const repoFile = (rel: string) => readFileSync(new URL(rel, REPO), "utf8");
const URDF = repoFile("assets/urdf/marengo.urdf");
const RECORD_NAME = "2026-10-04-gravity-20261004T113836Z-20261004T113951Z.json";
const RECORD = repoFile(`docs/commissioning/calibrations/${RECORD_NAME}`);
const RECORD_REL = path.posix.join(path.posix.dirname(GRAVITY_INDEX_PATH), RECORD_NAME);
const [PITCH, ROLL, , ELBOW] = MASTER_JOINTS;
const CHAIN = [...MASTER_JOINTS];

function index(entry: Record<string, unknown> = {}): string {
  return JSON.stringify({
    version: 1,
    joints: { [PITCH]: { record: RECORD_NAME, urdf_gravity_sha256: gravityModelFingerprint(URDF), ...entry } },
  });
}

/** In-memory repo: relative path → content; anything else is ENOENT. */
function reader(files: Record<string, string>) {
  return async (rel: string): Promise<string> => {
    const body = files[rel];
    if (body === undefined) throw Object.assign(new Error(`ENOENT: ${rel}`), { code: "ENOENT" });
    return body;
  };
}

const resolve = (files: Record<string, string>, urdf = URDF) =>
  resolveTauFactors({ urdf, joints: CHAIN, readLocal: reader(files) });

describe("gravity-model fingerprint", () => {
  it("ignores limits, comments and whitespace", () => {
    const edited = URDF.replace('upper="3.25"', 'upper="3.007"')
      .replace("<robot", "<!-- taught --><robot")
      .replace(/\n {2}<joint/g, "\n\n    <joint");
    assert.equal(gravityModelFingerprint(edited), gravityModelFingerprint(URDF));
  });

  it("changes with a link COM, a mass or a joint origin", () => {
    const base = gravityModelFingerprint(URDF);
    for (const edited of [
      URDF.replace('xyz="-0.00403 0.06933 0.01041"', 'xyz="-0.00403 0.06933 0.01141"'),
      URDF.replace('<mass value="0.3910"/>', '<mass value="0.4000"/>'),
      URDF.replace('<origin xyz="0 0 -0.23036" rpy="0 0 0"/>', '<origin xyz="0 0 -0.24" rpy="0 0 0"/>'),
    ]) {
      assert.notEqual(edited, URDF);
      assert.notEqual(gravityModelFingerprint(edited), base);
    }
  });
});

describe("τ guard factor selection", () => {
  it("a fitted, applied joint gets 1 + max(3σ_A/A, 0.15): pitch 1.15", async () => {
    const f = await resolve({ [GRAVITY_INDEX_PATH]: index(), [RECORD_REL]: RECORD });
    // A 2.661 ± 0.05357 Nm: 3σ/A = 6.0 % < 15 % → the 1.15 floor.
    assert.equal(f.at(PITCH, { [PITCH]: 1.57 }), 1.15);
    assert.equal(f.at(ELBOW, { [PITCH]: 1.57 }), UNVERIFIED_TAU_FACTOR);
    assert.match(f.report.join("\n"), /right_shoulder_pitch: × 1\.15 calibrated/);
    assert.match(f.report.join("\n"), /right_elbow_pitch: × 1\.6 unverified \(not in the index\)/);
  });

  it("a wide σ_A sets the factor above the floor", async () => {
    const wide = JSON.parse(RECORD);
    wide.groups[0].fit.sigma_a_nm = 0.2661;
    const f = await resolve({ [GRAVITY_INDEX_PATH]: index(), [RECORD_REL]: JSON.stringify(wide) });
    assert.equal(f.at(PITCH, {}), 1.3);
  });

  it("the calibrated factor holds only at the fit's fixed pose of the other joints", async () => {
    const f = await resolve({ [GRAVITY_INDEX_PATH]: index(), [RECORD_REL]: RECORD });
    assert.equal(f.at(PITCH, { [PITCH]: 0.5, [ROLL]: 0 }), 1.15);
    assert.equal(f.at(PITCH, { [PITCH]: 0.5, [ELBOW]: 0.4 }), UNVERIFIED_TAU_FACTOR);
  });
});

describe("τ guard factor fails closed to 1.6", () => {
  const unverified = async (files: Record<string, string>, reason: RegExp, urdf = URDF) => {
    const f = await resolve(files, urdf);
    for (const j of CHAIN) assert.equal(f.at(j, {}), UNVERIFIED_TAU_FACTOR, j);
    assert.match(f.report.join("\n"), reason);
  };

  it("without the index", () => unverified({ [RECORD_REL]: RECORD }, /index missing/));

  it("with an unparseable or wrong-version index", async () => {
    await unverified({ [GRAVITY_INDEX_PATH]: "{", [RECORD_REL]: RECORD }, /index unreadable/);
    await unverified({ [GRAVITY_INDEX_PATH]: JSON.stringify({ version: 2, joints: {} }), [RECORD_REL]: RECORD }, /version/);
  });

  it("when the indexed record is missing", () => unverified({ [GRAVITY_INDEX_PATH]: index() }, /record .* missing/));

  it("when the Pi URDF's gravity model changed since the fit was applied (stale)", () =>
    unverified(
      { [GRAVITY_INDEX_PATH]: index(), [RECORD_REL]: RECORD },
      /stale/,
      URDF.replace('<origin xyz="0 0 -0.23036" rpy="0 0 0"/>', '<origin xyz="0 0 -0.24" rpy="0 0 0"/>'),
    ));

  it("when the fitted COMs are not in the URDF (fit not applied)", async () => {
    // Index pinned to a URDF that never got the patch: fingerprint matches, COMs do not.
    const cad = URDF.replace('xyz="-0.00403 0.06933 0.01041"', 'xyz="-0.00378 0.06933 -0.00005"');
    await unverified(
      { [GRAVITY_INDEX_PATH]: index({ urdf_gravity_sha256: gravityModelFingerprint(cad) }), [RECORD_REL]: RECORD },
      /not applied: right_shoulder_pitch_link/,
      cad,
    );
  });

  it("when the record or its group was not accepted, or carries no σ_A", async () => {
    for (const mutate of [
      (r: { accepted: boolean }) => {
        r.accepted = false;
      },
      (r: { groups: { accepted: boolean }[] }) => {
        r.groups[0].accepted = false;
      },
      (r: { groups: { fit: Record<string, unknown> }[] }) => {
        delete r.groups[0].fit.sigma_a_nm;
      },
    ]) {
      const record = JSON.parse(RECORD);
      mutate(record);
      await unverified({ [GRAVITY_INDEX_PATH]: index(), [RECORD_REL]: JSON.stringify(record) }, /unverified/);
    }
  });

  it("when the record path leaves the calibrations directory", () =>
    unverified({ [GRAVITY_INDEX_PATH]: index({ record: "../../../etc/x.json" }), [RECORD_REL]: RECORD }, /record name/));
});

describe("repo calibration index", () => {
  it("pins the committed URDF and calibrates the pitch to 1.15", async () => {
    const f = await resolveTauFactors({
      urdf: URDF,
      joints: CHAIN,
      readLocal: async (rel) => repoFile(rel),
    });
    assert.equal(f.at(PITCH, {}), 1.15, f.report.join("\n"));
    for (const j of CHAIN.slice(1)) assert.equal(f.at(j, {}), UNVERIFIED_TAU_FACTOR, j);
  });
});
