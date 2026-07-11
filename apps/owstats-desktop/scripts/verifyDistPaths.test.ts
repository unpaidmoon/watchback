import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { verifyDistPaths } from "./verifyDistPaths.mjs";

const root = await mkdtemp(path.join(tmpdir(), "owstats-dist-paths-"));
try {
  const dist = path.join(root, "dist");
  await mkdir(dist);
  await writeFile(path.join(dist, "clean.js"), "const url='/assets/heroes/ana.png';");
  await verifyDistPaths(dist, { repoRoot: root });

  for (const forbidden of ["/@fs/D:/repo/file.png", "D:/Codex/OW_data/file.png", "C:\\Users\\user\\file.png", "docs/reference/query.json", `${root.replaceAll("\\", "/")}/assets/file.png`]) {
    await writeFile(path.join(dist, "bad.js"), forbidden);
    await assert.rejects(() => verifyDistPaths(dist, { repoRoot: root }), /bad\.js/, `scanner rejects ${forbidden}`);
  }
} finally {
  await rm(root, { recursive: true, force: true });
}
