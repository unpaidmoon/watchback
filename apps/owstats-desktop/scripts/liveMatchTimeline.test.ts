import assert from "node:assert/strict";
import { chooseLiveTimelineRefresh, createLiveMatchTimelineController, mergeLiveTimelinePage, type SeasonDataRevisionEvent } from "../src/features/matches/liveMatchTimeline.ts";
import { chooseMatchIdForLoadedMatches } from "../src/shared/matchesPageCache.ts";
import { createMatchFavoriteDraft, matchFavoriteDraftReducer, shouldResetMatchFavoriteDraft } from "../src/features/matches/matchFavoriteState.ts";

const listeners = new Set<(event: SeasonDataRevisionEvent) => void>();
const timers: Array<() => void> = [];
let activeAccountScope = "account-a";
const controller = createLiveMatchTimelineController({
  listen: async (listener) => { listeners.add(listener); return () => listeners.delete(listener); },
  getActiveAccountScope: async () => activeAccountScope,
  schedule: (callback) => { timers.push(callback); return callback; },
  cancel: (timer) => { const index = timers.indexOf(timer as () => void); if (index >= 0) timers.splice(index, 1); },
});

await controller.start();
const notifications: SeasonDataRevisionEvent[] = [];
const unsubscribe = controller.subscribe((event) => notifications.push(event));
const emit = async (event: SeasonDataRevisionEvent) => {
  listeners.forEach((listener) => listener(event));
  await Promise.resolve();
};
const flush = () => { while (timers.length) timers.shift()!(); };

await emit({ accountScope: "account-b", season: 23, revision: 1, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
flush();
assert.equal(notifications.length, 0, "another account cannot invalidate the active timeline");

await emit({ accountScope: "account-a", season: 23, revision: 1, phase: "list", newMatches: 1, pendingDetails: 2, status: "running" });
await emit({ accountScope: "account-a", season: 23, revision: 2, phase: "list", newMatches: 2, pendingDetails: 1, status: "running" });
assert.equal(notifications.length, 0, "list bursts are debounced");
flush();
assert.equal(notifications.length, 1);
assert.equal(notifications[0]?.revision, 2, "the newest revision wins inside the debounce window");
assert.equal(controller.getRevision("account-a", 23), 2, "revision remains available when no page is mounted");
assert.equal(controller.getRevisionToken("account-a"), "23:2", "revision tokens encode season identity instead of using a collision-prone max");
assert.equal(chooseLiveTimelineRefresh(notifications[0]!, 23, true), "refresh", "the top of the matching season refreshes immediately");
assert.equal(chooseLiveTimelineRefresh(notifications[0]!, 23, false), "queue", "a scrolled matching season preserves its viewport");
assert.equal(chooseLiveTimelineRefresh(notifications[0]!, 22, true), "ignore", "another selected season is isolated");

await emit({ accountScope: "account-a", season: 23, revision: 2, phase: "detail", newMatches: 0, pendingDetails: 0, status: "running" });
flush();
assert.equal(notifications.length, 1, "duplicate or late revisions are ignored");

await emit({ accountScope: "account-a", season: 23, revision: 3, phase: "detail", newMatches: 0, pendingDetails: 0, status: "running" });
await emit({ accountScope: "account-a", season: 22, revision: 8, phase: "list", newMatches: 4, pendingDetails: 0, status: "running" });
flush();
assert.deepEqual(notifications.slice(1).map((event) => [event.season, event.revision]), [[23, 3], [22, 8]], "season streams remain isolated");

activeAccountScope = "account-b";
await emit({ accountScope: "account-b", season: 23, revision: 7, phase: "list", newMatches: 1, pendingDetails: 0, status: "completed" });
flush();
assert.equal(controller.getRevision("account-b", 23), 7, "the controller revalidates active account scope on delivery");

unsubscribe();
controller.stop();
assert.equal(listeners.size, 0, "global listener releases its event subscription");
assert.deepEqual(
  mergeLiveTimelinePage(
    [{ matchId: "new", value: 1 }, { matchId: "older", value: 1 }],
    [{ matchId: "new", value: 2 }],
    "detail",
  ),
  [{ matchId: "new", value: 2 }, { matchId: "older", value: 1 }],
  "detail refresh updates rows without dropping the loaded tail",
);
assert.deepEqual(
  mergeLiveTimelinePage([{ matchId: "older" }], [{ matchId: "new" }], "list"),
  [{ matchId: "new" }, { matchId: "older" }],
  "list refresh prepends new rows while preserving pagination context",
);

const loadedPages = Array.from({ length: 120 }, (_, index) => ({ matchId: `match-${index}`, detailVersion: 1 }));
const listRefresh = mergeLiveTimelinePage(loadedPages, [{ matchId: "brand-new", detailVersion: 1 }], "list");
assert.equal(listRefresh.length, 121, "a first-page refresh preserves every already loaded pagination row");
assert.equal(listRefresh.at(-1)?.matchId, "match-119", "the pagination tail remains stable after a prepend");
assert.equal(
  chooseMatchIdForLoadedMatches("match-80", listRefresh, true),
  "match-80",
  "a live prepend does not steal the user's selected match",
);
const detailRefresh = mergeLiveTimelinePage(listRefresh, [{ matchId: "match-80", detailVersion: 2 }], "detail");
assert.equal(detailRefresh.length, 121, "a detail refresh never truncates loaded pages");
assert.equal(detailRefresh.find((row) => row.matchId === "match-80")?.detailVersion, 2, "detail refresh replaces the affected row");
assert.equal(detailRefresh.at(-1)?.matchId, "match-119", "detail refresh preserves the pagination tail");

const dirtyDraft = matchFavoriteDraftReducer(createMatchFavoriteDraft({ note: "saved" }), { type: "edit", note: "unsaved draft" });
assert.equal(
  shouldResetMatchFavoriteDraft({ previousMatchId: "match-80", matchId: "match-80", favorite: { note: "server update" }, state: dirtyDraft }),
  false,
  "same-match live detail refresh cannot overwrite an unsaved note draft",
);

activeAccountScope = "account-c";
await controller.start();
const burstQueries: number[] = [];
const burstUnsubscribe = controller.subscribe((event) => burstQueries.push(event.revision));
for (let revision = 1; revision <= 5; revision += 1) {
  await emit({ accountScope: "account-c", season: 23, revision, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
}
assert.equal(burstQueries.length, 0, "event bursts do not query before the debounce window closes");
flush();
assert.deepEqual(burstQueries, [5], "a five-mode event burst causes one subscriber query with the latest revision");

const visibleLatencySamples: number[] = [];
for (let revision = 6; revision <= 45; revision += 1) {
  const started = performance.now();
  await emit({ accountScope: "account-c", season: 23, revision, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
  flush();
  const refreshed = mergeLiveTimelinePage([{ matchId: "older" }], [{ matchId: `new-${revision}` }], "list");
  assert.equal(refreshed[0]?.matchId, `new-${revision}`);
  visibleLatencySamples.push(performance.now() - started);
}
visibleLatencySamples.sort((a, b) => a - b);
const visibleP95 = visibleLatencySamples[Math.ceil(visibleLatencySamples.length * 0.95) - 1]!;
assert.ok(visibleP95 <= 150, `event-to-visible P95 ${visibleP95.toFixed(2)}ms exceeded 150ms`);
burstUnsubscribe();
controller.stop();
console.log("live match timeline tests passed");
