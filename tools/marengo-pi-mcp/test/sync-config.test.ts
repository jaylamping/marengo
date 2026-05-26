import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { directInstallRsyncLine } from "../src/tools/sync-config.js";

describe("bench config sync", () => {
  it("avoids chmod/chown metadata writes during direct /opt installs", async () => {
    assert.equal(
      directInstallRsyncLine,
      'rsync -r --no-owner --no-group --no-perms --omit-dir-times "$SRC/" "$DST/"',
    );
    assert.doesNotMatch(directInstallRsyncLine, /rsync -a/);
  });
});
