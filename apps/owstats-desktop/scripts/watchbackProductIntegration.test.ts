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
const databaseMigrations = source("src-tauri/src/database/migrations.rs");
const tauriLibrary = source("src-tauri/src/lib.rs");
const tauriMain = source("src-tauri/src/main.rs");
const readme = repositorySource("README.md");
const cargoManifest = source("src-tauri/Cargo.toml");
const commands = source("src-tauri/src/commands.rs");
const frontendApi = source("src/shared/api.ts");
const onboarding = source("src/pages/OnboardingPage.tsx");

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

assert.match(readme, /^# Watchback$/m);
assert.match(readme, /^Watchback 是一款面向 Windows 的《守望先锋》本地战绩与复盘工具/m);
assert.match(cargoManifest, /description = "Watchback desktop app"/);
assert.match(cargoManifest, /authors = \["Watchback"\]/);
assert.match(commands, /const PRODUCT_FEEDBACK_URL: &str = "https:\/\/support\.qq\.com\/product\/801522";/);
assert.match(commands, /pub fn open_product_feedback\(\) -> Result<\(\), String>/);
assert.match(tauriLibrary, /commands::open_product_feedback/);
assert.match(frontendApi, /invoke<void>\("open_product_feedback"\)/);
assert.match(onboarding, /label="意见反馈"/);
assert.match(onboarding, /openProductFeedback\(\)/);
assert.match(databaseMigrations, /请升级 Watchback 后再打开此数据/);
assert.match(
  tauriMain,
  /owstats_desktop_lib::handle_desktop_startup_result\(\s*owstats_desktop_lib::run\(\),\s*show_desktop_startup_failure,\s*\)/s,
);
assert.match(tauriMain, /if exit != 0 \{\s*std::process::exit\(exit\);\s*\}/s);
assert.doesNotMatch(tauriMain, /owstats_desktop_lib::run\(\)\s*\.expect\(/);
assert.match(
  tauriLibrary,
  /present\(startup_failure_message\(&error\)\);\s*1/s,
);

assert.match(credentialStore, /OWStatsLocal\/DeepSeekApiKey/);
assert.match(credentialStore, /wide\("OWStatsLocal"\)/);

console.log("Watchback product integration contracts passed");
