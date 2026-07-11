import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const configUrl = new URL("../runtime-config/ow-lookups.json", import.meta.url);
const config = JSON.parse(readFileSync(configUrl, "utf8")) as {
  attrs?: Record<string, { text?: string; type?: string }>;
  heroPerkList?: Record<string, Array<{ id?: string; guid?: string; name?: string; desc?: string }>>;
  perkList?: Array<{ id?: string; guid?: string; name?: string; desc?: string }>;
  heroAttrList?: Array<{ valueGuid?: string; valueText?: string; valueType?: string }>;
};

const forbiddenKey = /^(?:token|customerToken|customer_token|cookie|authorization|credential|storage)$/i;

function assertNoSensitiveKeys(value: unknown, path = "root"): void {
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNoSensitiveKeys(item, `${path}[${index}]`));
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    assert.equal(forbiddenKey.test(key), false, `runtime config must not contain sensitive key ${path}.${key}`);
    assertNoSensitiveKeys(child, `${path}.${key}`);
  }
}

assertNoSensitiveKeys(config);
assert.deepEqual(Object.keys(config).sort(), ["attrs", "heroAttrList", "heroPerkList", "perkList"]);
assert.ok(config.attrs && Object.keys(config.attrs).length > 0, "runtime config should include stat attributes");
assert.equal(config.attrs?.["603482350067646509"]?.text, "单独消灭");
assert.equal(config.attrs?.["603482350067646509"]?.type, "通用数据");

const perks = [...Object.values(config.heroPerkList ?? {}).flat(), ...(config.perkList ?? [])];
for (const perk of perks) {
  assert.deepEqual(
    Object.keys(perk).sort(),
    Object.keys(perk).filter((key) => ["desc", "guid", "id", "name"].includes(key)).sort(),
    "perk entries should contain only fields read by the UI",
  );
}
const reaperPerk = perks.find((perk) => perk.id?.toUpperCase() === "0X0CC8000000002830");
assert.match(reaperPerk?.name ?? "", /萦绕幽灵/);
assert.match(`${reaperPerk?.name ?? ""}${reaperPerk?.desc ?? ""}`, /幽灵形态/);

assert.ok(Array.isArray(config.heroAttrList), "runtime config should include hero attribute fallbacks");
for (const attr of config.heroAttrList ?? []) {
  assert.deepEqual(
    Object.keys(attr).sort(),
    Object.keys(attr).filter((key) => ["valueGuid", "valueText", "valueType"].includes(key)).sort(),
    "hero attribute entries should contain only fields read by the UI",
  );
}
