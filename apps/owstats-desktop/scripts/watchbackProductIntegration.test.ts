import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function source(relativePath: string) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), "utf8");
}

function repositorySource(relativePath: string) {
  return readFileSync(new URL(`../../../${relativePath}`, import.meta.url), "utf8");
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
const careerMmrQuery = source("src-tauri/src/career_mmr/query.rs");
const databaseMigrations = source("src-tauri/src/database/migrations.rs");
const tauriLibrary = source("src-tauri/src/lib.rs");
const agents = repositorySource("AGENTS.md");
const readme = repositorySource("README.md");
const design = repositorySource("design.md");
const currentDevelopment = repositorySource("docs/current-development-status.md");
const stabilization = repositorySource("docs/stabilization-progress.md");
const mainThreadLog = repositorySource("docs/superpowers/specs/main-thread-log.md");
const brandDesign = repositorySource(
  "docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md",
);
const cargoManifest = source("src-tauri/Cargo.toml");

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

assert.match(agents, /^# Watchback 协作说明$/m);
assert.match(readme, /^# Watchback$/m);
assert.match(readme, /^Watchback 是一款面向 Windows 的《守望先锋》本地数据分析桌面应用/m);
assert.match(design, /^# Watchback 设计规范$/m);
assert.match(design, /本文件是 Watchback 新页面与页面重构的视觉控制规范/);
assert.match(currentDevelopment, /^# Watchback 当前开发状态$/m);
assert.match(stabilization, /^# Watchback 稳定化进度$/m);
assert.match(mainThreadLog, /^# Watchback Main Thread Log$/m);
assert.match(mainThreadLog, /durable ledger for the Watchback main-thread workflow/);
assert.match(brandDesign, /为 Watchback 建立正式的产品名称与图标体系/);
assert.match(cargoManifest, /description = "Watchback desktop app"/);
assert.match(cargoManifest, /authors = \["Watchback"\]/);
assert.match(careerMmrQuery, /Watchback 本地模型推算，并非暴雪官方隐藏分。/);
assert.match(databaseMigrations, /请升级 Watchback 后再打开此数据/);
assert.match(tauriLibrary, /failed to run Watchback/);

assert.match(credentialStore, /OWStatsLocal\/DeepSeekApiKey/);
assert.match(credentialStore, /wide\("OWStatsLocal"\)/);

console.log("Watchback product integration contracts passed");
