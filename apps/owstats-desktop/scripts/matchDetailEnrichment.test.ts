import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import assetManifest from "../../../assets/manifest.json" with { type: "json" };
import { perkAssetUrls, resolveHeroStatInfo, resolvePerkInfo } from "../src/shared/assets.ts";

const rustModels = readFileSync(new URL("../src-tauri/src/models.rs", import.meta.url), "utf8");
const rustQueries = [
  "../src-tauri/src/queries/mod.rs",
  "../src-tauri/src/queries/normal_detail.rs",
  "../src-tauri/src/queries/arena_detail.rs",
].map((path) => readFileSync(new URL(path, import.meta.url), "utf8")).join("\n");
const sharedTypes = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");
const sharedAssets = readFileSync(new URL("../src/shared/assets.ts", import.meta.url), "utf8");

assert.equal(rustModels.includes("pub struct PlayerPerk"), true, "Rust model should expose player perk records");
assert.equal(rustModels.includes("pub struct HeroStat"), true, "Rust model should expose hero stat records");
assert.equal(rustModels.includes("pub perks: Vec<PlayerPerk>"), true, "PlayerRow should include perks");
assert.equal(rustModels.includes("pub hero_stats: Vec<HeroStat>"), true, "PlayerRow should include hero stats");

assert.equal(rustQueries.includes("parse_player_perks(item)"), true, "queries should parse perks from raw player rows");
assert.equal(rustQueries.includes("parse_hero_stats(item)"), true, "queries should parse statMap from raw player rows");
assert.equal(rustQueries.includes('item.get("statMap")'), true, "hero stats should read statMap");
assert.equal(rustQueries.includes('item.get("perks")'), true, "perks should read perks");

assert.equal(sharedTypes.includes("export type PlayerPerk"), true, "frontend types should expose PlayerPerk");
assert.equal(sharedTypes.includes("export type HeroStat"), true, "frontend types should expose HeroStat");
assert.equal(sharedTypes.includes("perks: PlayerPerk[]"), true, "frontend PlayerRow should include perks");
assert.equal(sharedTypes.includes("heroStats: HeroStat[]"), true, "frontend PlayerRow should include hero stats");
assert.equal(sharedAssets.includes("runtime-config/ow-lookups.json"), true, "hero stat labels should use the tracked runtime lookup config");
assert.equal(sharedAssets.includes("docs/reference"), false, "frontend assets must not import ignored reference data");
assert.equal(sharedAssets.includes("/@fs/"), false, "frontend assets must not emit Vite filesystem URLs");
assert.equal(sharedAssets.includes("attrs?: Record<string"), true, "asset helpers should understand ow_config_cache attrs");

const reaperPerk = resolvePerkInfo("920986123797276720");
assert.equal(reaperPerk?.id, "0x0CC8000000002830", "decimal raw perk guid should resolve to hex query_tool id");
assert.equal(reaperPerk?.name, "萦绕幽灵", "perk name should strip the description split marker");
assert.match(reaperPerk?.description ?? "", /幽灵形态/, "perk description should be available");
assert.match(reaperPerk?.iconUrl ?? "", /^\/assets\/perks\//, "perk icon should resolve to a stable asset URL");
assert.deepEqual(perkAssetUrls(["920986123797276720"]), [reaperPerk?.iconUrl], "perkAssetUrls should expose icon candidates");

const soloKills = resolveHeroStatInfo("603482350067646509");
assert.equal(soloKills?.label, "单独消灭", "hero stat guid should resolve through ow_config_cache attrs");
assert.equal(soloKills?.type, "通用数据", "hero stat type should resolve through ow_config_cache attrs");

const manifest = assetManifest as { perks?: Array<{ key: string; source_url: string; local_path: string }> };
assert.ok((manifest.perks?.length ?? 0) >= 200, "asset manifest should include downloaded perk icons");
assert.ok(
  manifest.perks?.some((perk) => perk.key === "0x0CC8000000002830" && perk.local_path.endsWith("perks/0x0CC8000000002830.png")),
  "asset manifest should map the verified perk to a local icon path",
);
