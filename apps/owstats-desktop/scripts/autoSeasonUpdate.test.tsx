import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SeasonSyncWorkbench } from "../src/features/command-center/SeasonSyncWorkbench";

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const backendSource = readFileSync(new URL("../src-tauri/src/auto_season_update.rs", import.meta.url), "utf8");
const commandsSource = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const libSource = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const commandCenterStyles = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");

const html = renderToStaticMarkup(createElement(SeasonSyncWorkbench, {
  selectedSeason: 23,
  minimumSyncSeason: 15,
  seasonStatuses: [],
  credentialReady: true,
  targetSyncAllowed: true,
  accountOperationBusy: false,
  syncingSeason: null,
  syncingMode: null,
  progress: null,
  syncError: null,
  syncNotice: null,
  autoUpdateEnabled: true,
  autoUpdateSaving: false,
  backgroundStatuses: {},
  onSelectSeason: () => undefined,
  onFullFetch: () => undefined,
  onQuickUpdate: () => undefined,
  onRetryBackground: () => undefined,
  onAutoUpdateChange: () => undefined,
}));

assert.match(html, /<strong>自动更新<\/strong>/);
assert.match(html, /type="checkbox" checked=""/);
assert.match(onboardingSource, /saveUserSettings\(\{ \.\.\.previous, autoUpdateSeason: enabled \}\)/);
assert.match(onboardingSource, /setUserSettings\(previous\)/, "failed saves restore the previous switch value");
assert.match(backendSource, /SELECT MAX\(season\) FROM matches/);
assert.match(backendSource, /quick_list_update_with_credentials_and_target_paths_base_url/);
assert.match(backendSource, /active_job_count\(season\)/);
assert.match(commandsSource, /switch_account[\s\S]*?auto_season_update::spawn/);
assert.match(libSource, /\.setup\([\s\S]*?auto_season_update::spawn/);
assert.match(commandCenterStyles, /\.seasonAutoUpdateToggle input\s*\{[\s\S]*?min-block-size:\s*20px !important;/, "switch track overrides the global 44px input minimum");

console.log("auto season update tests passed");
