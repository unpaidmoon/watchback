import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const scripts = packageJson.scripts as Record<string, string>;

for (const name of ["test:resources", "test:unit", "test:components"]) {
  assert.equal(typeof scripts[name], "string", `${name} should be a named verification command`);
}
assert.equal(scripts.pretest, "npm run prepare:assets");
assert.equal(scripts.prebuild, "npm run prepare:assets");
assert.equal(
  scripts.test,
  "npm run test:resources && npm run test:unit && npm run test:components && npm run test:text",
);
