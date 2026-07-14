import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolveHeroDisplayName, resolveMapDisplayName } from "../src/shared/assets";

const rankingPage = await readFile(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");

assert.equal(
  rankingPage.includes("resolveHeroDisplayName"),
  true,
  "ranking rows should resolve legacy hero GUIDs to bundled display names",
);
assert.equal(
  rankingPage.includes("resolveMapDisplayName"),
  true,
  "ranking rows should resolve legacy map GUIDs to bundled display names",
);

assert.equal(resolveHeroDisplayName("207165582859042819", "207165582859042819"), "猎空");
assert.equal(resolveHeroDisplayName("207165582859042938", "207165582859042938"), "D.Va");
assert.equal(resolveHeroDisplayName("unknown-guid", "Tracer"), "猎空");
assert.equal(resolveHeroDisplayName("209999999999999999", "209999999999999999"), "未知英雄");
assert.equal(resolveMapDisplayName("576460752303423700", "576460752303423700"), "国王大道");
assert.equal(resolveMapDisplayName("oasis", "Oasis"), "绿洲城");
assert.equal(resolveMapDisplayName("shambali-monastery", "Shambali Monastery"), "香巴里寺院");
assert.equal(resolveMapDisplayName("579999999999999999", "579999999999999999"), "未知地图");

console.log("ranking display-name tests passed");
