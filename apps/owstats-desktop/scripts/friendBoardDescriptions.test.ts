import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const analyticsSource = readFileSync(new URL("../src-tauri/src/analytics.rs", import.meta.url), "utf8");

assert.equal(
  analyticsSource.includes('"受疗不低，千疗击杀更高。"'),
  true,
  "healing conversion board should use a compact description",
);

assert.equal(
  analyticsSource.includes('"平均受到治疗量不低，同时每 1000 点受疗带来的击杀更高。"'),
  false,
  "healing conversion board should not keep the old long two-line description",
);
