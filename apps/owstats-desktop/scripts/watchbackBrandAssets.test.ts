import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const desktopRoot = resolve(import.meta.dirname, "..");
const brandRoot = resolve(desktopRoot, "assets/brand/watchback");
const iconRoot = resolve(desktopRoot, "src-tauri/icons");

const required = [
  resolve(brandRoot, "watchback-mark.svg"),
  resolve(brandRoot, "watchback-mark-monochrome.svg"),
  resolve(brandRoot, "watchback-mark-inverse.svg"),
  resolve(brandRoot, "watchback-mark-16.svg"),
  resolve(brandRoot, "watchback-mark-24.svg"),
  resolve(brandRoot, "watchback-logo.svg"),
  resolve(brandRoot, "watchback-logo-with-descriptor.svg"),
  resolve(brandRoot, "watchback-review-sheet.svg"),
  resolve(iconRoot, "watchback-icon-source.svg"),
  resolve(iconRoot, "icon.ico"),
];

for (const file of required) {
  assert.equal(existsSync(file), true, `missing Watchback asset: ${file}`);
}

const standard = readFileSync(required[0], "utf8");
assert.match(standard, /viewBox="0 0 96 96"/);
assert.equal((standard.match(/<path\b/g) ?? []).length, 3);
assert.equal((standard.match(/<circle\b/g) ?? []).length, 1);
assert.equal((standard.match(/stroke="#202220"/g) ?? []).length, 3);
assert.match(standard, /fill="#4B22D1"/);
assert.match(standard, /stroke-width="10"/);
assert.match(standard, /stroke-linecap="round"/);

const mono = readFileSync(required[1], "utf8");
assert.doesNotMatch(mono, /#4B22D1/i);
assert.match(mono, /fill="#202220"/);

const inverse = readFileSync(required[2], "utf8");
assert.doesNotMatch(inverse, /#4B22D1/i);
assert.match(inverse, /stroke="#F4F6F0"/);
assert.match(inverse, /fill="#F4F6F0"/);

for (const file of required.slice(0, 9)) {
  const source = readFileSync(file, "utf8");
  assert.doesNotMatch(source, /(?:data:image|<script|(?:href|src)="https?:\/\/|C:\\|D:\\|\/@fs\/)/i);
  assert.doesNotMatch(source, /(?:filter=|linearGradient|radialGradient|<image\b)/i);
}

const descriptor = readFileSync(required[6], "utf8");
assert.match(descriptor, />守望战绩工具<\/text>/);

const small16 = readFileSync(required[3], "utf8");
assert.match(small16, /stroke-width="11\.5"/);

const iconSource = readFileSync(required[8], "utf8");
assert.match(iconSource, /<rect width="96" height="96" rx="22" fill="#F4F6F0"\/>/);

console.log("Watchback brand asset contracts passed");
