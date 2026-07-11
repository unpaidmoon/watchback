import { readFileSync } from "node:fs";

import { arenaRankAssetUrl, heroAssetUrl, heroAssetUrls, mapAssetUrl, mapAssetUrls, normalizeAssetKey, rankAssetUrl } from "../src/shared/assets.ts";

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

const assetManifest = JSON.parse(
  readFileSync(new URL("../../../assets/manifest.json", import.meta.url), "utf8"),
) as { heroes: Array<{ key: string; cn_name: string }> };
const shion = assetManifest.heroes.find((hero) => hero.key === "shion");

assertEqual(shion?.cn_name, "死怨", "Shion uses the localized Chinese hero name");

assertEqual(normalizeAssetKey("Soldier: 76"), "soldier-76", "normalizes punctuation and spaces");
assertEqual(normalizeAssetKey("  King's Row  "), "kings-row", "normalizes apostrophes");
assertEqual(normalizeAssetKey(""), null, "empty key returns null");

assertEqual(
  heroAssetUrl("Ana"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/heroes/ana.png",
  "hero asset URL uses normalized key",
);
assertEqual(
  heroAssetUrl("安娜"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/heroes/ana.png",
  "hero asset URL maps Chinese names through manifest",
);
assertEqual(heroAssetUrl(""), null, "missing hero key returns null");
assertEqual(
  JSON.stringify(heroAssetUrls(["0x02E000000000013B", "安娜"])),
  JSON.stringify([
    "/@fs/D:/Codex/OW_data/owstats-local/assets/heroes/0x02e000000000013b.png",
    "/@fs/D:/Codex/OW_data/owstats-local/assets/heroes/ana.png",
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
    `/@fs/D:/Codex/OW_data/owstats-local/assets/heroes/${key}.png`,
    `hero asset URL maps ${heroName}`,
  );
}

assertEqual(
  mapAssetUrl("Blizzard World"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/maps/blizzard-world.jpg",
  "map asset URL uses normalized key",
);
assertEqual(
  mapAssetUrl("帕拉伊苏"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/maps/paraiso.jpg",
  "map asset URL maps localized names",
);
assertEqual(
  JSON.stringify(mapAssetUrls(["guid-paraiso", "帕拉伊苏"])),
  JSON.stringify([
    "/@fs/D:/Codex/OW_data/owstats-local/assets/maps/guid-paraiso.jpg",
    "/@fs/D:/Codex/OW_data/owstats-local/assets/maps/paraiso.jpg",
  ]),
  "map asset URL candidates try guid then localized name",
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
    `/@fs/D:/Codex/OW_data/owstats-local/assets/maps/${key}.jpg`,
    `map asset URL maps ${mapName}`,
  );
}
assertEqual(mapAssetUrl(null), null, "missing map key returns null");

assertEqual(
  rankAssetUrl("champion"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/ranks/champion.png",
  "rank asset URL uses stable rank key",
);
assertEqual(
  rankAssetUrl("missing-rank"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/ranks/unranked.png",
  "unknown rank falls back to unranked",
);
assertEqual(
  arenaRankAssetUrl("master"),
  "/@fs/D:/Codex/OW_data/owstats-local/assets/stadium/ranks/fight/6-master.png",
  "arena rank asset URL uses the full fight rank icon",
);
assertEqual(
  arenaRankAssetUrl("master")?.includes("pure"),
  false,
  "arena rank asset URL should not use pure fight rank icons",
);
