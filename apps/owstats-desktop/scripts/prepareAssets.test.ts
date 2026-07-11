import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { prepareAssets, replaceDirectoryAtomically } from "./prepareAssets.mjs";

type Fixture = {
  appRoot: string;
  repoRoot: string;
  rootManifest: Record<string, unknown>;
  runtimeConfig: Record<string, unknown>;
};

async function makeFixture(): Promise<Fixture> {
  const repoRoot = await mkdtemp(path.join(tmpdir(), "owstats-assets-"));
  const appRoot = path.join(repoRoot, "apps", "owstats-desktop");
  const rootManifest = {
    heroes: [{ local_path: "data/assets/overwatch/heroes/ana.png" }],
    maps: [],
    perks: [],
    achievements: [{ local_path: "data/assets/overwatch/achievements/badge.png" }],
  };
  const runtimeConfig = { attrs: { damage: { text: "伤害", type: "通用数据" } } };

  await mkdir(path.join(repoRoot, "assets", "heroes"), { recursive: true });
  await mkdir(path.join(repoRoot, "assets", "achievements"), { recursive: true });
  await mkdir(path.join(repoRoot, "assets", "token-style"), { recursive: true });
  await mkdir(path.join(repoRoot, "assets", "ranks", "open-wide"), { recursive: true });
  await mkdir(path.join(repoRoot, "assets", "stadium", "ranks", "fight"), { recursive: true });
  await mkdir(path.join(appRoot, "runtime-config"), { recursive: true });
  await writeFile(path.join(repoRoot, "assets", "heroes", "ana.png"), "hero");
  await writeFile(path.join(repoRoot, "assets", "achievements", "badge.png"), "achievement");
  await writeFile(path.join(repoRoot, "assets", "token-style", "unreferenced.png"), "unreferenced");
  await writeFile(path.join(repoRoot, "assets", "ranks", "bronze.png"), "rank");
  await writeFile(path.join(repoRoot, "assets", "ranks", "open-wide", "c1.png"), "wide");
  await writeFile(path.join(repoRoot, "assets", "ranks", "open-wide", "bronze.png"), "rank-wide");
  await writeFile(path.join(repoRoot, "assets", "stadium", "ranks", "fight", "bronze.png"), "fight");
  await writeFile(path.join(repoRoot, "assets", "stadium", "ranks", "fight", "bronze-pure.png"), "fight-pure");
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await writeJson(path.join(repoRoot, "assets", "ranks", "manifest.json"), {
    ranks: [{ file: "bronze.png", wideFile: "open-wide/bronze.png" }],
    openWideFiles: ["open-wide/c1.png"],
  });
  await writeJson(path.join(repoRoot, "assets", "stadium", "manifest.json"), {
    fightRanks: [{
      localPath: "assets/stadium/ranks/fight/bronze.png",
      pureLocalPath: "assets/stadium/ranks/fight/bronze-pure.png",
    }],
    modTrait: [],
  });
  await writeJson(path.join(appRoot, "runtime-config", "ow-lookups.json"), runtimeConfig);
  return { appRoot, repoRoot, rootManifest, runtimeConfig };
}

async function outputSnapshot(root: string): Promise<Array<{ path: string; sha256: string }>> {
  const files: string[] = [];
  async function walk(directory: string): Promise<void> {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const fullPath = path.join(directory, entry.name);
      if (entry.isDirectory()) await walk(fullPath);
      else files.push(fullPath);
    }
  }
  await walk(root);
  const snapshot = await Promise.all(files.sort().map(async (file) => ({
    path: path.relative(root, file).replaceAll("\\", "/"),
    sha256: createHash("sha256").update(await readFile(file)).digest("hex"),
  })));
  return snapshot;
}

async function writeJson(file: string, value: unknown): Promise<void> {
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, `${JSON.stringify(value, null, 2)}\n`);
}

async function withFixture(run: (fixture: Fixture) => Promise<void>): Promise<void> {
  const fixture = await makeFixture();
  try {
    await run(fixture);
  } finally {
    await rm(fixture.repoRoot, { recursive: true, force: true });
  }
}

await withFixture(async ({ appRoot, repoRoot }) => {
  await prepareAssets({ appRoot, repoRoot });
  assert.equal(await readFile(path.join(appRoot, "public", "assets", "heroes", "ana.png"), "utf8"), "hero");
  assert.equal(await readFile(path.join(appRoot, "public", "assets", "ranks", "bronze.png"), "utf8"), "rank");
  assert.equal(
    await readFile(path.join(appRoot, "public", "assets", "token-style", "unreferenced.png"), "utf8"),
    "unreferenced",
  );
  assert.equal(
    await readFile(path.join(appRoot, "public", "assets", "stadium", "ranks", "fight", "bronze.png"), "utf8"),
    "fight",
  );
  assert.deepEqual(
    JSON.parse(await readFile(path.join(appRoot, "public", "assets", "config", "ow-lookups.json"), "utf8")),
    { attrs: { damage: { text: "伤害", type: "通用数据" } } },
  );
});

await withFixture(async ({ appRoot, repoRoot }) => {
  await prepareAssets({ appRoot, repoRoot });
  const first = await outputSnapshot(path.join(appRoot, "public", "assets"));
  await prepareAssets({ appRoot, repoRoot });
  const second = await outputSnapshot(path.join(appRoot, "public", "assets"));
  assert.deepEqual(second, first);
});

await withFixture(async ({ appRoot, repoRoot }) => {
  const outsideFile = path.join(repoRoot, "outside-secret.txt");
  const linkedFile = path.join(repoRoot, "assets", "heroes", "linked.png");
  await writeFile(outsideFile, "outside");
  await symlink(outsideFile, linkedFile, "file");
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /符号链接.*heroes[\\/]linked\.png/i);
});

await withFixture(async ({ appRoot }) => {
  const outputRoot = path.join(appRoot, "public", "assets");
  const stagingRoot = path.join(appRoot, "public", ".assets-stage-test");
  await mkdir(outputRoot, { recursive: true });
  await mkdir(stagingRoot, { recursive: true });
  await writeFile(path.join(outputRoot, "old.txt"), "old");
  await writeFile(path.join(stagingRoot, "new.txt"), "new");
  let renameCalls = 0;
  await assert.rejects(
    replaceDirectoryAtomically(outputRoot, stagingRoot, {
      rename: async (from: string, to: string) => {
        renameCalls += 1;
        if (renameCalls === 2) throw new Error("injected rename failure");
        const { rename } = await import("node:fs/promises");
        await rename(from, to);
      },
    }),
    /injected rename failure/,
  );
  assert.equal(await readFile(path.join(outputRoot, "old.txt"), "utf8"), "old");
  await assert.rejects(readFile(path.join(stagingRoot, "new.txt")), /ENOENT/);
});

await withFixture(async ({ appRoot, repoRoot, rootManifest }) => {
  rootManifest.heroes = {};
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*heroes.*数组/i);
});

await withFixture(async ({ appRoot, repoRoot, rootManifest }) => {
  rootManifest.heroes = ["invalid-entry"];
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*heroes\[0\].*对象/i);
});

await withFixture(async ({ appRoot, repoRoot }) => {
  await writeJson(path.join(repoRoot, "assets", "ranks", "manifest.json"), {
    ranks: [{ file: "bronze.png", wideFile: "open-wide/missing.png" }],
    openWideFiles: [],
  });
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/ranks\/manifest\.json.*wideFile.*missing\.png/i);
});

await withFixture(async ({ appRoot, repoRoot }) => {
  await writeJson(path.join(repoRoot, "assets", "stadium", "manifest.json"), {
    fightRanks: [{
      localPath: "assets/stadium/ranks/fight/bronze.png",
      pureLocalPath: "assets/stadium/ranks/fight/missing-pure.png",
    }],
    modTrait: [],
  });
  await assert.rejects(
    prepareAssets({ appRoot, repoRoot }),
    /assets\/stadium\/manifest\.json.*pureLocalPath.*missing-pure\.png/i,
  );
});

await withFixture(async ({ appRoot, repoRoot }) => {
  const staleFile = path.join(appRoot, "public", "assets", "stale", "old.png");
  await mkdir(path.dirname(staleFile), { recursive: true });
  await writeFile(staleFile, "stale");

  await prepareAssets({ appRoot, repoRoot });

  await assert.rejects(readFile(staleFile), /ENOENT/);
});

await withFixture(async ({ appRoot, repoRoot, rootManifest }) => {
  rootManifest.heroes = [{ local_path: "data/assets/overwatch/heroes/missing.png" }];
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*heroes\/missing\.png/i);
});

await withFixture(async ({ appRoot, repoRoot, rootManifest }) => {
  rootManifest.heroes = [{ local_path: "C:\\private\\hero.png" }];
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*绝对路径/i);
});

await withFixture(async ({ appRoot, repoRoot, rootManifest }) => {
  rootManifest.heroes = [{ local_path: "/@fs/D:/private/hero.png" }];
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*禁止 \/@fs\/ 路径/i);
});

await withFixture(async ({ appRoot, repoRoot, rootManifest }) => {
  rootManifest.heroes = [{ local_path: "data/assets/overwatch/../private.png" }];
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*\.\./i);
});

await withFixture(async ({ appRoot, repoRoot, runtimeConfig }) => {
  runtimeConfig.safe = [{ nested: { customerToken: "redacted" } }];
  await writeJson(path.join(appRoot, "runtime-config", "ow-lookups.json"), runtimeConfig);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /ow-lookups\.json.*customerToken/i);
});
