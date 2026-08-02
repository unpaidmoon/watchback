import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SeasonSyncWorkbench } from "../src/features/command-center/SeasonSyncWorkbench";

const html = renderToStaticMarkup(createElement(SeasonSyncWorkbench, {
  selectedSeason: 23,
  minimumSyncSeason: 15,
  seasonStatuses: [],
  credentialReady: true,
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
console.log("auto season update tests passed");
