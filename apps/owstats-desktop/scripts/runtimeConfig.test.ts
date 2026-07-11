import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const configUrl = new URL("../runtime-config/ow-lookups.json", import.meta.url);
const config = JSON.parse(readFileSync(configUrl, "utf8")) as {
  attrs?: Record<string, { text?: string; type?: string }>;
  heroPerkList?: Record<string, Array<{ id?: string; guid?: string; name?: string; desc?: string }>>;
  perkList?: Array<{ id?: string; guid?: string; name?: string; desc?: string }>;
  heroAttrList?: Array<{ valueGuid?: string; valueText?: string; valueType?: string }>;
};

function isSensitiveKey(key: string): boolean {
  const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
  return (
    normalized.endsWith("token") ||
    normalized === "cookie" ||
    normalized === "cookies" ||
    normalized === "authorization" ||
    normalized === "credential" ||
    normalized === "credentials" ||
    normalized === "storage"
  );
}

function assertNoSensitiveKeys(value: unknown, path = "root"): void {
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNoSensitiveKeys(item, `${path}[${index}]`));
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    assert.equal(isSensitiveKey(key), false, `runtime config must not contain sensitive key ${path}.${key}`);
    assertNoSensitiveKeys(child, `${path}.${key}`);
  }
}

for (const key of [
  "accessToken",
  "refreshToken",
  "apiToken",
  "api_token",
  "customerToken",
  "customer_token",
  "cookie",
  "cookies",
  "authorization",
  "credential",
  "credentials",
  "storage",
]) {
  assert.throws(() => assertNoSensitiveKeys({ safe: [{ nested: { [key]: "redacted" } }] }));
}
assert.doesNotThrow(() => assertNoSensitiveKeys({ description: "ordinary token-style value" }));

assertNoSensitiveKeys(config);
assert.deepEqual(Object.keys(config).sort(), ["attrs", "heroAttrList", "heroPerkList", "perkList"]);
assert.ok(config.attrs && Object.keys(config.attrs).length > 0, "runtime config should include stat attributes");
assert.equal(config.attrs?.["603482350067646509"]?.text, "单独消灭");
assert.equal(config.attrs?.["603482350067646509"]?.type, "通用数据");

const perks = [...Object.values(config.heroPerkList ?? {}).flat(), ...(config.perkList ?? [])];
assert.ok(Object.keys(config.heroPerkList ?? {}).length > 0, "runtime config should include hero perk groups");
assert.ok(Object.values(config.heroPerkList ?? {}).flat().length > 0, "runtime config should include hero perks");
assert.ok((config.perkList?.length ?? 0) > 0, "runtime config should include the global perk list");
for (const perk of perks) {
  assert.deepEqual(
    Object.keys(perk).sort(),
    Object.keys(perk).filter((key) => ["desc", "guid", "id", "name"].includes(key)).sort(),
    "perk entries should contain only fields read by the UI",
  );
  assert.ok(perk.id || perk.guid, "perk entries should include an id or guid consumed by the UI");
}
assert.ok(perks.some((perk) => perk.name), "runtime config should include perk display names");
const reaperPerk = perks.find((perk) => perk.id?.toUpperCase() === "0X0CC8000000002830");
assert.match(reaperPerk?.name ?? "", /萦绕幽灵/);
assert.match(`${reaperPerk?.name ?? ""}${reaperPerk?.desc ?? ""}`, /幽灵形态/);

assert.ok(Array.isArray(config.heroAttrList), "runtime config should include hero attribute fallbacks");
assert.ok((config.heroAttrList?.length ?? 0) > 0, "runtime config should include hero attribute entries");
assert.equal(
  new Set(config.heroAttrList?.map((attr) => attr.valueGuid)).size,
  config.heroAttrList?.length,
  "hero attributes should be first-wins unique by valueGuid",
);
for (const attr of config.heroAttrList ?? []) {
  assert.deepEqual(
    Object.keys(attr).sort(),
    Object.keys(attr).filter((key) => ["valueGuid", "valueText", "valueType"].includes(key)).sort(),
    "hero attribute entries should contain only fields read by the UI",
  );
  assert.ok(attr.valueGuid, "hero attribute entries should include valueGuid consumed by the UI");
  assert.ok(attr.valueText, "hero attribute entries should include valueText consumed by the UI");
}
assert.equal(config.heroAttrList?.find((attr) => attr.valueGuid === "603482350067647444")?.valueType, "通用数据");
