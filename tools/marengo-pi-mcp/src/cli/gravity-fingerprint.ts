/**
 * Print the gravity-model fingerprint of a URDF (default: the repo's assets/urdf/marengo.urdf)
 * for `urdf_gravity_sha256` in docs/commissioning/calibrations/applied-gravity.json.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { defaultLocalRoot } from "../config.js";
import { gravityModelFingerprint } from "../tau-factor.js";

const urdf = process.argv[2] ?? path.join(defaultLocalRoot(), "assets/urdf/marengo.urdf");
console.log(gravityModelFingerprint(readFileSync(urdf, "utf8")));
