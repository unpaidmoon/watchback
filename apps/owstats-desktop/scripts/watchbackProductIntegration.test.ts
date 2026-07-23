import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function source(relativePath: string) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), "utf8");
}

const tauriConfig = JSON.parse(source("src-tauri/tauri.conf.json")) as {
  productName: string;
  identifier: string;
  app: { windows: Array<{ title: string }> };
};
const html = source("index.html");
const app = source("src/App.tsx");
const shell = source("src/styles/shell.css");
const fixture = source("scripts/matchWorkbenchVisualFixture.tsx");
const acceptanceFixture = source("scripts/uiVisualAcceptance.fixture.ts");
const credentialStore = source("src-tauri/src/deepseek_key.rs");

assert.equal(tauriConfig.productName, "Watchback");
assert.equal(tauriConfig.app.windows[0]?.title, "Watchback");
assert.equal(tauriConfig.identifier, "local.owstats.desktop");

assert.match(html, /<title>Watchback<\/title>/);
assert.match(app, /const watchbackMarkUrl = new URL\("\.\.\/assets\/brand\/watchback\/watchback-mark\.svg", import\.meta\.url\)\.href;/);
assert.match(app, /<img src=\{watchbackMarkUrl\} alt="" \/>/);
assert.match(app, /<strong className="appBrandName">Watchback<\/strong>/);
assert.match(app, /<p>守望战绩工具<\/p>/);
assert.doesNotMatch(app, /Sparkles|OWStatsLocal/);

assert.match(shell, /\.appBrand > \.phase6ProductLogo\s*\{[^}]*border-radius:\s*var\(--radius-control\)/s);
assert.match(shell, /\.appBrand > \.phase6ProductLogo\s*\{[^}]*background:\s*#F4F6F0/s);
assert.match(shell, /\.appBrand > \.phase6ProductLogo img\s*\{[^}]*width:\s*32px[^}]*height:\s*32px/s);
assert.doesNotMatch(shell, /\.appBrand > \.phase6ProductLogo\s*\{[^}]*background:\s*var\(--action-primary\)/s);

assert.match(fixture, /<strong className="appBrandName">Watchback<\/strong>/);
assert.doesNotMatch(fixture, /OWStatsLocal/);
assert.match(acceptanceFixture, /productName:\s*"Watchback"/);

assert.match(credentialStore, /OWStatsLocal\/DeepSeekApiKey/);
assert.match(credentialStore, /wide\("OWStatsLocal"\)/);

console.log("Watchback product integration contracts passed");
