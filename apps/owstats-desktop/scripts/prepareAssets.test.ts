import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { prepareAssets } from "./prepareAssets.mjs";

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
  };
  const runtimeConfig = { attrs: { damage: { text: "伤害", type: "通用数据" } } };

  await mkdir(path.join(repoRoot, "assets", "heroes"), { recursive: true });
  await mkdir(path.join(repoRoot, "assets", "ranks", "open-wide"), { recursive: true });
  await mkdir(path.join(repoRoot, "assets", "stadium", "ranks", "fight"), { recursive: true });
  await mkdir(path.join(appRoot, "runtime-config"), { recursive: true });
  await writeFile(path.join(repoRoot, "assets", "heroes", "ana.png"), "hero");
  await writeFile(path.join(repoRoot, "assets", "ranks", "bronze.png"), "rank");
  await writeFile(path.join(repoRoot, "assets", "ranks", "open-wide", "c1.png"), "wide");
  await writeFile(path.join(repoRoot, "assets", "stadium", "ranks", "fight", "bronze.png"), "fight");
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await writeJson(path.join(repoRoot, "assets", "ranks", "manifest.json"), {
    ranks: [{ file: "bronze.png" }],
    openWideFiles: ["open-wide/c1.png"],
  });
  await writeJson(path.join(repoRoot, "assets", "stadium", "manifest.json"), {
    fightRanks: [{ localPath: "assets/stadium/ranks/fight/bronze.png" }],
    modTrait: [],
  });
  await writeJson(path.join(appRoot, "runtime-config", "ow-lookups.json"), runtimeConfig);
  return { appRoot, repoRoot, rootManifest, runtimeConfig };
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
    await readFile(path.join(appRoot, "public", "assets", "stadium", "ranks", "fight", "bronze.png"), "utf8"),
    "fight",
  );
  assert.deepEqual(
    JSON.parse(await readFile(path.join(appRoot, "public", "assets", "config", "ow-lookups.json"), "utf8")),
    { attrs: { damage: { text: "伤害", type: "通用数据" } } },
  );
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
  rootManifest.heroes = [{ local_path: "data/assets/overwatch/../private.png" }];
  await writeJson(path.join(repoRoot, "assets", "manifest.json"), rootManifest);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /assets\/manifest\.json.*\.\./i);
});

await withFixture(async ({ appRoot, repoRoot, runtimeConfig }) => {
  runtimeConfig.safe = [{ nested: { customerToken: "redacted" } }];
  await writeJson(path.join(appRoot, "runtime-config", "ow-lookups.json"), runtimeConfig);
  await assert.rejects(prepareAssets({ appRoot, repoRoot }), /ow-lookups\.json.*customerToken/i);
});
