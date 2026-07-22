import { readFileSync } from "node:fs";

import { arenaRankAssetUrl, heroAssetUrl, heroAssetUrls, mapAssetUrl, mapAssetUrls, normalizeAssetKey, normalizeStadiumAssetPath, rankAssetUrl, resolveArenaModTraitInfo } from "../src/shared/assets.ts";

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

const assetManifest = JSON.parse(
  readFileSync(new URL("../../../assets/manifest.json", import.meta.url), "utf8"),
) as { heroes: Array<{ key: string; cn_name: string }> };
const shion = assetManifest.heroes.find((hero) => hero.key === "shion");
const viteConfig = readFileSync(new URL("../vite.config.ts", import.meta.url), "utf8");
const assetsInventory = readFileSync(new URL("../runtime-config/assets-inventory.json", import.meta.url), "utf8");

assertEqual(viteConfig.includes("server.fs"), false, "Vite does not allow machine-specific filesystem roots");
assertEqual(viteConfig.includes("assets/stadium/manifest.json"), true, "build removes the non-runtime stadium source manifest from dist");
assertEqual(assetsInventory.includes("achievements/achievement-glow-tier-2.webp"), true, "tier II achievement glow is packaged");
assertEqual(assetsInventory.includes("achievements/achievement-glow-tier-3.webp"), true, "tier III achievement glow is packaged");

assertEqual(shion?.cn_name, "死怨", "Shion uses the localized Chinese hero name");

assertEqual(normalizeAssetKey("Soldier: 76"), "soldier-76", "normalizes punctuation and spaces");
assertEqual(normalizeAssetKey("  King's Row  "), "kings-row", "normalizes apostrophes");
assertEqual(normalizeAssetKey(""), null, "empty key returns null");

assertEqual(
  heroAssetUrl("Ana"),
  "/assets/heroes/ana.png",
  "hero asset URL uses normalized key",
);
assertEqual(
  heroAssetUrl("安娜"),
  "/assets/heroes/ana.png",
  "hero asset URL maps Chinese names through manifest",
);
assertEqual(heroAssetUrl(""), null, "missing hero key returns null");
assertEqual(
  heroAssetUrl("207165582859043131"),
  "/assets/heroes/ana.png",
  "legacy numeric hero id maps through the bundled runtime lookup",
);
assertEqual(
  JSON.stringify(heroAssetUrls(["0x02E000000000013B", "安娜"])),
  JSON.stringify([
    "/assets/heroes/0x02e000000000013b.png",
    "/assets/heroes/ana.png",
  ]),
  "hero asset URL candidates try guid then localized name",
);

const localizedHeroExpectations: Array<[string, string]> = [
  ["飞天猫", "jetpack-cat"],
  ["埃姆雷", "emre"],
  ["安燃", "anran"],
  ["西拉", "sierra"],
  ["瑞稀", "mizuki"],
  ["金驭", "domina"],
  ["无漾", "wuyang"],
  ["斩仇", "vendetta"],
];

for (const [heroName, key] of localizedHeroExpectations) {
  assertEqual(
    heroAssetUrl(heroName),
    `/assets/heroes/${key}.png`,
    `hero asset URL maps ${heroName}`,
  );
}

assertEqual(
  mapAssetUrl("Blizzard World"),
  "/assets/maps/blizzard-world.jpg",
  "map asset URL uses normalized key",
);
assertEqual(
  mapAssetUrl("帕拉伊苏"),
  "/assets/maps/paraiso.jpg",
  "map asset URL maps localized names",
);
assertEqual(
  JSON.stringify(mapAssetUrls(["guid-paraiso", "帕拉伊苏"])),
  JSON.stringify([
    "/assets/maps/guid-paraiso.jpg",
    "/assets/maps/paraiso.jpg",
  ]),
  "map asset URL candidates try guid then localized name",
);
assertEqual(
  mapAssetUrl("576460752303428223"),
  "/assets/maps/oasis3.png",
  "stadium oasis garden map guid uses manual preview",
);
assertEqual(
  mapAssetUrl("576460752303428212"),
  "/assets/maps/serenza.png",
  "stadium serenza map guid uses manual preview",
);
assertEqual(
  mapAssetUrl("576460752303428683"),
  "/assets/maps/lijiangtower2.png",
  "stadium lijiang night market map guid uses manual preview",
);
assertEqual(
  mapAssetUrl("576460752303428209"),
  "/assets/maps/powder-keg-mine.jpg",
  "stadium powder keg mine map guid uses its mapped preview",
);
assertEqual(
  mapAssetUrl("576460752303428266"),
  "/assets/maps/arena-victoriae.jpg",
  "stadium arena victoriae map guid uses its mapped preview",
);
assertEqual(
  mapAssetUrl("霓虹枢纽"),
  "/assets/maps/neon-junction.jpg",
  "neon junction localized name uses its mapped preview",
);
assertEqual(
  mapAssetUrl("576460752303427628"),
  "/assets/maps/neon-junction.jpg",
  "neon junction map guid uses its mapped preview",
);

const localizedMapExpectations: Array<[string, string]> = [
  ["中城", "midtown"],
  ["鲁纳塞彼", "runasapi"],
  ["监测站：直布罗陀", "watchpoint-gibraltar"],
  ["斗兽场", "colosseo"],
];

for (const [mapName, key] of localizedMapExpectations) {
  assertEqual(
    mapAssetUrl(mapName),
    `/assets/maps/${key}.jpg`,
    `map asset URL maps ${mapName}`,
  );
}
assertEqual(mapAssetUrl(null), null, "missing map key returns null");

assertEqual(
  rankAssetUrl("champion"),
  "/assets/ranks/champion.png",
  "rank asset URL uses stable rank key",
);
assertEqual(
  rankAssetUrl("missing-rank"),
  "/assets/ranks/unranked.png",
  "unknown rank falls back to unranked",
);
assertEqual(
  arenaRankAssetUrl("master"),
  "/assets/stadium/ranks/fight/6-master.png",
  "arena rank asset URL uses the full fight rank icon",
);
assertEqual(
  arenaRankAssetUrl("master")?.includes("pure"),
  false,
  "arena rank asset URL should not use pure fight rank icons",
);

assertEqual(normalizeStadiumAssetPath("assets/stadium/mod-trait/icon.png"), "/assets/stadium/mod-trait/icon.png", "stadium path maps to the stable asset root");
assertEqual(resolveArenaModTraitInfo("0x0CC800000000FFFF"), null, "unmapped arena id remains unresolved for the UI fallback");
for (const unsafePath of [
  "/@fs/D:/repo/assets/stadium/icon.png",
  "D:/repo/assets/stadium/icon.png",
  "../assets/stadium/icon.png",
  "assets/stadium/../icon.png",
  "assets/stadium/%2e%2e/icon.png",
  "assets/stadium/folder%2ficon.png",
  "assets/stadium/C:/x.png",
  "assets/stadium//server/icon.png",
  "assets/stadium/folder\\icon.png",
  "assets/stadium/./icon.png",
  "assets/stadium/folder\u0000icon.png",
]) {
  assertEqual(normalizeStadiumAssetPath(unsafePath), null, `stadium path rejects unsafe input ${unsafePath}`);
}
