import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("../src/features/matches/FilterBar.tsx", import.meta.url), "utf8");

assert.equal(source.includes("filterActions"), false, "FilterBar should not render date preset action buttons");
assert.equal(source.includes("shortcut("), false, "FilterBar should not keep date shortcut handlers");
