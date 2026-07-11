import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(appRoot, "../..");
const cache = JSON.parse(await readFile(resolve(repoRoot, "docs/reference/ow_config_cache.json"), "utf8"));
const queryTool = JSON.parse(await readFile(resolve(repoRoot, "docs/reference/res/query_tool.json"), "utf8"));

const pick = (value, keys) => Object.fromEntries(keys.flatMap((key) => (value?.[key] == null ? [] : [[key, value[key]]])));
const perkFields = ["id", "guid", "name", "desc"];
const attrFields = ["valueGuid", "valueText", "valueType"];

const runtimeConfig = {
  attrs: Object.fromEntries(Object.entries(cache.attrs ?? {}).map(([key, value]) => [key, pick(value, ["text", "type"])])),
  heroPerkList: Object.fromEntries(
    Object.entries(queryTool.heroPerkList ?? {}).map(([key, perks]) => [key, perks.map((perk) => pick(perk, perkFields))]),
  ),
  perkList: (queryTool.perkList ?? []).map((perk) => pick(perk, perkFields)),
  heroAttrList: (queryTool.heroAttrList ?? []).map((attr) => pick(attr, attrFields)),
};

const output = resolve(appRoot, "runtime-config/ow-lookups.json");
await mkdir(dirname(output), { recursive: true });
await writeFile(output, `${JSON.stringify(runtimeConfig, null, 2)}\n`, "utf8");
console.log(`wrote ${output}`);
