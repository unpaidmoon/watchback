import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";

const appRoot = resolve(import.meta.dirname, "..");
const tauriConfig = JSON.parse(await readFile(resolve(appRoot, "src-tauri/tauri.conf.json"), "utf8"));
const csp = String(tauriConfig.app?.security?.csp ?? "");
const imageDirective = csp
  .split(";")
  .map((directive) => directive.trim())
  .find((directive) => directive.startsWith("img-src "));

assert.ok(imageDirective, "the installed app must declare an img-src policy");
const imageSources = new Set(imageDirective.split(/\s+/).slice(1));

const appSource = await readFile(resolve(appRoot, "src/App.tsx"), "utf8");
const brandAsset = resolve(appRoot, "assets/brand/watchback/watchback-mark.svg");
assert.match(appSource, /new URL\([^)]*watchback-mark\.svg[^)]*import\.meta\.url\)/);
assert.ok((await stat(brandAsset)).size <= 4096, "the Watchback mark exercises Vite's default data-URL inlining path");
assert.ok(imageSources.has("data:"), "img-src must allow the inlined Watchback mark and query-page data-URL avatars");

const profileCardSource = await readFile(resolve(appRoot, "src/features/player-query/PlayerProfileCard.tsx"), "utf8");
assert.match(profileCardSource, /`data:\$\{image\.mimeType\};base64,/);

const commandCenterSource = await readFile(resolve(appRoot, "src/pages/OnboardingPage.tsx"), "utf8");
assert.match(commandCenterSource, /<img src=\{profile\.avatarUrl\}/);
assert.ok(imageSources.has("https:"), "img-src must allow HTTPS account avatars in the command center");

console.log("installed image policy tests passed");
