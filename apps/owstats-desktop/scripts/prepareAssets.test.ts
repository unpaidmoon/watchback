import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, readdir, rename, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { prepareAssets, replaceDirectoryAtomically } from "./prepareAssets.mjs";

type Fixture = {
  appRoot: string;
  repoRoot: string;
  rootManifest: Record<string, unknown>;
  runtimeConfig: Record<string, unknown>;
  inventory: { version: number; files: string[] };
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
  const inventory = {
    version: 1,
    files: [
      "achievements/badge.png",
      "heroes/ana.png",
      "manifest.json",
      "ranks/bronze.png",
      "ranks/manifest.json",
      "ranks/open-wide/bronze.png",
      "ranks/open-wide/c1.png",
      "stadium/manifest.json",
      "stadium/ranks/fight/bronze-pure.png",
      "stadium/ranks/fight/bronze.png",
    ],
  };
  await writeJson(path.join(appRoot, "runtime-config", "assets-inventory.json"), inventory);
  return { appRoot, repoRoot, rootManifest, runtimeConfig, inventory };
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

await withFixture(async ({ appRoot, repoRoot, inventory }) => {
  const result = await prepareAssets({ appRoot, repoRoot });
  assert.equal(await readFile(path.join(appRoot, "public", "assets", "heroes", "ana.png"), "utf8"), "hero");
  assert.equal(await readFile(path.join(appRoot, "public", "assets", "ranks", "bronze.png"), "utf8"), "rank");
  await assert.rejects(readFile(path.join(appRoot, "public", "assets", "token-style", "unreferenced.png")), /ENOENT/);
  assert.equal(
    await readFile(path.join(appRoot, "public", "assets", "stadium", "ranks", "fight", "bronze.png"), "utf8"),
    "fight",
  );
  await assert.rejects(readFile(path.join(appRoot, "public", "assets", "config", "ow-lookups.json")), /ENOENT/);
  assert.equal(result.fileCount, inventory.files.length, "prepared output count includes tracked assets only");
});

await withFixture(async ({ appRoot, repoRoot }) => {
  await prepareAssets({ appRoot, repoRoot });
  const first = await outputSnapshot(path.join(appRoot, "public", "assets"));
  await prepareAssets({ appRoot, repoRoot });
  const second = await outputSnapshot(path.join(appRoot, "public", "assets"));
  assert.deepEqual(second, first);
});

await withFixture(async ({ appRoot, repoRoot, inventory }) => {
  const outsideFile = path.join(repoRoot, "outside-secret.txt");
  const linkedFile = path.join(repoRoot, "assets", "heroes", "linked.png");
  await writeFile(outsideFile, "outside");
  await symlink(outsideFile, linkedFile, "file");
  inventory.files.push("heroes/linked.png");
  inventory.files.sort();
  await writeJson(path.join(appRoot, "runtime-config", "assets-inventory.json"), inventory);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /符号链接.*heroes[\\/]linked\.png/i);
});

await withFixture(async ({ appRoot, repoRoot, inventory }) => {
  inventory.files.push("heroes/missing.png");
  inventory.files.sort();
  await writeJson(path.join(appRoot, "runtime-config", "assets-inventory.json"), inventory);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets-inventory\.json.*heroes\/missing\.png.*不存在/i);
});

await withFixture(async ({ appRoot, repoRoot, inventory }) => {
  inventory.files = inventory.files.filter((file) => file !== "heroes/ana.png");
  await writeJson(path.join(appRoot, "runtime-config", "assets-inventory.json"), inventory);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets-inventory\.json.*未列出.*heroes\/ana\.png/i);
});

await withFixture(async ({ appRoot, repoRoot, inventory }) => {
  inventory.files.push("../outside.png");
  inventory.files.sort();
  await writeJson(path.join(appRoot, "runtime-config", "assets-inventory.json"), inventory);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets-inventory\.json.*\.\./i);
});

await withFixture(async ({ appRoot, repoRoot, inventory }) => {
  inventory.files.push(inventory.files[0]);
  inventory.files.sort();
  await writeJson(path.join(appRoot, "runtime-config", "assets-inventory.json"), inventory);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets-inventory\.json.*重复/i);
});

await withFixture(async ({ appRoot, repoRoot }) => {
  const assetsRoot = path.join(repoRoot, "assets");
  const externalAssets = path.join(repoRoot, "external-assets");
  await rename(assetsRoot, externalAssets);
  await symlink(externalAssets, assetsRoot, "junction");
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets.*根目录.*符号链接/i);
});

await withFixture(async ({ appRoot, repoRoot }) => {
  const runtimeRoot = path.join(appRoot, "runtime-config");
  const externalRuntime = path.join(repoRoot, "external-runtime");
  await rename(runtimeRoot, externalRuntime);
  await symlink(externalRuntime, runtimeRoot, "junction");
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /runtime-config.*根目录.*符号链接/i);
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
  assert.equal(renameCalls, 3, "non-transient rename errors must not retry");
});

await withFixture(async ({ appRoot }) => {
  const outputRoot = path.join(appRoot, "public", "assets");
  const stagingRoot = path.join(appRoot, "public", ".assets-stage-transient");
  await mkdir(outputRoot, { recursive: true });
  await mkdir(stagingRoot, { recursive: true });
  await writeFile(path.join(outputRoot, "old.txt"), "old");
  await writeFile(path.join(stagingRoot, "new.txt"), "new");
  let renameCalls = 0;
  const { rename: nativeRename } = await import("node:fs/promises");

  await replaceDirectoryAtomically(outputRoot, stagingRoot, {
    rename: async (from: string, to: string) => {
      renameCalls += 1;
      if (renameCalls <= 2) {
        const error = new Error(`transient rename ${renameCalls}`) as NodeJS.ErrnoException;
        error.code = renameCalls === 1 ? "EPERM" : "EBUSY";
        throw error;
      }
      await nativeRename(from, to);
    },
    sleep: async () => {},
  });

  assert.equal(await readFile(path.join(outputRoot, "new.txt"), "utf8"), "new");
  assert.equal(renameCalls, 4);
});

await withFixture(async ({ appRoot }) => {
  const publicRoot = path.join(appRoot, "public");
  const outputRoot = path.join(publicRoot, "assets");
  const stagingRoot = path.join(publicRoot, ".assets-stage-permanent");
  await mkdir(outputRoot, { recursive: true });
  await mkdir(stagingRoot, { recursive: true });
  await writeFile(path.join(outputRoot, "old.txt"), "old");
  await writeFile(path.join(stagingRoot, "new.txt"), "new");
  let stagingRenameAttempts = 0;
  const { rename: nativeRename } = await import("node:fs/promises");

  await assert.rejects(
    replaceDirectoryAtomically(outputRoot, stagingRoot, {
      rename: async (from: string, to: string) => {
        if (from === stagingRoot) {
          stagingRenameAttempts += 1;
          const error = new Error("permanent rename lock") as NodeJS.ErrnoException;
          error.code = "EPERM";
          throw error;
        }
        await nativeRename(from, to);
      },
      sleep: async () => {},
    }),
    /permanent rename lock/,
  );

  assert.equal(stagingRenameAttempts, 5);
  assert.equal(await readFile(path.join(outputRoot, "old.txt"), "utf8"), "old");
  await assert.rejects(readFile(path.join(stagingRoot, "new.txt")), /ENOENT/);
  assert.deepEqual((await readdir(publicRoot)).filter((name) => name.includes(".backup-")), []);
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
