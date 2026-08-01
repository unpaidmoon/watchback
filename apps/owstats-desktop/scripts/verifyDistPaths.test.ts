import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { verifyDistPaths } from "./verifyDistPaths.mjs";

const root = await mkdtemp(path.join(tmpdir(), "owstats-dist-paths-"));
try {
  const dist = path.join(root, "dist");
  await mkdir(dist);
  await writeFile(path.join(dist, "clean.js"), "const asset='/assets/heroes/ana.png'; const remote='https://example.com';");
  await writeFile(path.join(dist, "binary.png"), Buffer.from([0, 255, ...Buffer.from("E:/outside-repository/file.png")]));
  await verifyDistPaths(dist, { repoRoot: root });

  await writeFile(path.join(dist, "oversized.js"), "x".repeat(500_001));
  await assert.rejects(
    () => verifyDistPaths(dist, { repoRoot: root }),
    /oversized\.js.*500001 B.*500000 B/,
    "scanner rejects an oversized production JavaScript chunk",
  );
  await rm(path.join(dist, "oversized.js"));

  await writeFile(path.join(dist, "phase6Components-test.js"), "x".repeat(250_001));
  await assert.rejects(
    () => verifyDistPaths(dist, { repoRoot: root }),
    /phase6Components-test\.js.*250001 B.*250000 B/,
    "scanner enforces the tighter shared component budget",
  );
  await rm(path.join(dist, "phase6Components-test.js"));

  for (const forbidden of [
    "/@fs/D:/repo/file.png",
    "D:/Codex/OW_data/file.png",
    "D:\\Codex\\OW_data\\file.png",
    "C:/Users/user/file.png",
    "C:\\Users\\user\\file.png",
    "E:/outside-repository/file.png",
    "docs/reference/query.json",
    `${root.replaceAll("\\", "/")}/assets/file.png`,
  ]) {
    await writeFile(path.join(dist, "bad.js"), forbidden);
    await assert.rejects(() => verifyDistPaths(dist, { repoRoot: root }), /bad\.js/, `scanner rejects ${forbidden}`);
  }
} finally {
  await rm(root, { recursive: true, force: true });
}
