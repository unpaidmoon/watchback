import assert from "node:assert/strict";
import {
  chooseLiveTimelineRefresh,
  coalesceLiveTimelineRevision,
  createLiveMatchTimelineController,
  reconcileLiveTimelinePage,
  type LiveTimelineInvalidation,
  type SeasonDataRevisionEvent,
} from "../src/features/matches/liveMatchTimeline.ts";
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
const event = (overrides: Partial<SeasonDataRevisionEvent> = {}): SeasonDataRevisionEvent => ({
  accountScope: "account-a",
  season: 23,
  revision: 1,
  phase: "detail",
  newMatches: 0,
  pendingDetails: 0,
  status: "running",
  ...overrides,
});

await controller.start();
const notifications: LiveTimelineInvalidation[] = [];
const unsubscribe = controller.subscribe((event) => notifications.push(event));
const emit = async (event: SeasonDataRevisionEvent) => {
  listeners.forEach((listener) => listener(event));
  await Promise.resolve();
};
const flush = () => { while (timers.length) timers.shift()!(); };

await emit({ accountScope: "account-b", season: 23, revision: 1, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
flush();
assert.equal(notifications.length, 0, "another account cannot invalidate the active timeline");

await emit({ accountScope: "account-a", season: 24, revision: 1, phase: "list", newMatches: 1, pendingDetails: 1, status: "running" });
await emit({ accountScope: "account-a", season: 24, revision: 2, phase: "detail", newMatches: 0, pendingDetails: 0, status: "running" });
flush();
const listThenDetail = notifications[0]!;
assert.deepEqual(
  reconcileLiveTimelinePage(
    { matches: [{ matchId: "older" }], nextCursor: null },
    { matches: [{ matchId: "new" }, { matchId: "older" }], nextCursor: null },
    listThenDetail,
  ).matches,
  [{ matchId: "new" }, { matchId: "older" }],
  "a detail revision cannot hide rows inserted by an earlier list revision in the same debounce window",
);
notifications.length = 0;

await emit({ accountScope: "account-a", season: 23, revision: 1, phase: "list", newMatches: 1, pendingDetails: 2, status: "running" });
await emit({ accountScope: "account-a", season: 23, revision: 2, phase: "list", newMatches: 2, pendingDetails: 1, status: "running" });
assert.equal(notifications.length, 0, "list bursts are debounced");
flush();
assert.equal(notifications.length, 1);
assert.equal(notifications[0]?.revision, 2, "the newest revision wins inside the debounce window");
assert.equal(notifications[0]?.newMatches, 3, "separate list commits accumulate their newly inserted match counts");
assert.equal(notifications[0]?.requiresListMerge, true, "a list burst retains list merge semantics");
assert.equal(controller.getRevision("account-a", 23), 2, "revision remains available when no page is mounted");
assert.equal(controller.getRevisionToken("account-a", 23), "23:2", "revision tokens encode season identity instead of using a collision-prone max");
assert.equal(chooseLiveTimelineRefresh(notifications[0]!, 23), "refresh", "the matching season refreshes immediately");
assert.equal(chooseLiveTimelineRefresh(notifications[0]!, 22), "ignore", "another selected season is isolated");

const relayedList = coalesceLiveTimelineRevision(undefined, event({ revision: 10, phase: "list", newMatches: 2, pendingDetails: 2 }));
const relayedDetail = coalesceLiveTimelineRevision(relayedList, event({ revision: 11, phase: "detail", pendingDetails: 1 }));
assert.equal(relayedDetail.requiresListMerge, true, "a newer detail request inherits an uncommitted list request's merge semantics");
assert.equal(relayedDetail.revision, 11, "request relay uses the newest database revision");
assert.deepEqual(
  reconcileLiveTimelinePage(
    { matches: [{ matchId: "older" }], nextCursor: null },
    { matches: [{ matchId: "new" }, { matchId: "older" }], nextCursor: null },
    relayedDetail,
  ).matches,
  [{ matchId: "new" }, { matchId: "older" }],
  "the request that wins a list/detail race still reveals every new row",
);

const listThenCompleted = coalesceLiveTimelineRevision(
  undefined,
  event({ revision: 12, phase: "list", newMatches: 4, pendingDetails: 4 }),
);
const completedInvalidation = coalesceLiveTimelineRevision(
  listThenCompleted,
  event({ revision: 13, phase: "completed", status: "completed" }),
);
assert.equal(completedInvalidation.phase, "completed", "the latest lifecycle phase is retained");
assert.equal(completedInvalidation.status, "completed", "the latest lifecycle status is retained");
assert.equal(completedInvalidation.requiresListMerge, true, "completion cannot downgrade pending list work to status-only");
assert.equal(completedInvalidation.newMatches, 4, "completion retains the accumulated new-match count");
assert.equal(chooseLiveTimelineRefresh(completedInvalidation, 23), "refresh", "completion keeps an accumulated list reset immediate");

const pureDetail = coalesceLiveTimelineRevision(undefined, event({ revision: 14, phase: "detail" }));
assert.equal(pureDetail.requiresListMerge, false, "a pure detail invalidation does not request a list merge");
assert.equal(chooseLiveTimelineRefresh(pureDetail, 23), "refresh", "a pure detail invalidation may refresh in place");
assert.deepEqual(
  reconcileLiveTimelinePage(
    { matches: [{ matchId: "older", value: 1 }], nextCursor: null },
    { matches: [{ matchId: "new", value: 1 }, { matchId: "older", value: 2 }], nextCursor: null },
    pureDetail,
  ).matches,
  [{ matchId: "older", value: 2 }],
  "a pure detail invalidation updates existing rows without inserting unseen rows",
);

await emit({ accountScope: "account-a", season: 23, revision: 2, phase: "detail", newMatches: 0, pendingDetails: 0, status: "running" });
flush();
assert.equal(notifications.length, 1, "duplicate or late revisions are ignored");

await emit({ accountScope: "account-a", season: 23, revision: 3, phase: "detail", newMatches: 0, pendingDetails: 0, status: "running" });
await emit({ accountScope: "account-a", season: 22, revision: 8, phase: "list", newMatches: 4, pendingDetails: 0, status: "running" });
flush();
assert.deepEqual(notifications.slice(1).map((event) => [event.season, event.revision]), [[23, 3], [22, 8]], "season streams remain isolated");
assert.deepEqual(notifications.slice(1).map((event) => event.requiresListMerge), [false, true], "season streams retain independent merge semantics");

activeAccountScope = "account-b";
await emit({ accountScope: "account-b", season: 23, revision: 7, phase: "list", newMatches: 1, pendingDetails: 0, status: "completed" });
flush();
assert.equal(controller.getRevision("account-b", 23), 7, "the controller revalidates active account scope on delivery");

unsubscribe();
controller.stop();
assert.equal(listeners.size, 0, "global listener releases its event subscription");
assert.deepEqual(
  reconcileLiveTimelinePage(
    {
      matches: [{ matchId: "new", value: 1 }, { matchId: "older", value: 1 }],
      nextCursor: { beginTs: 1, matchId: "older" },
    },
    {
      matches: [{ matchId: "new", value: 2 }],
      nextCursor: { beginTs: 2, matchId: "new" },
    },
    coalesceLiveTimelineRevision(undefined, event({ phase: "detail" })),
  ),
  {
    matches: [{ matchId: "new", value: 2 }, { matchId: "older", value: 1 }],
    nextCursor: { beginTs: 1, matchId: "older" },
    resetViewport: false,
  },
  "detail refresh updates rows without dropping the loaded tail",
);

const loadedPages = Array.from({ length: 120 }, (_, index) => ({ matchId: `match-${index}`, detailVersion: 1 }));
const resetCommit = reconcileLiveTimelinePage(
  { matches: loadedPages, nextCursor: { beginTs: 100, matchId: "old-tail" } },
  {
    matches: [{ matchId: "brand-new", detailVersion: 1 }],
    nextCursor: { beginTs: 200, matchId: "brand-new" },
  },
  coalesceLiveTimelineRevision(undefined, event({ phase: "list", newMatches: 1 })),
);
assert.deepEqual(
  resetCommit,
  {
    matches: [{ matchId: "brand-new", detailVersion: 1 }],
    nextCursor: { beginTs: 200, matchId: "brand-new" },
    resetViewport: true,
  },
  "a live list refresh enters a fresh first-page state instead of retaining previously loaded pages",
);
const detailRefresh = reconcileLiveTimelinePage(
  { matches: loadedPages, nextCursor: { beginTs: 100, matchId: "old-tail" } },
  {
    matches: [{ matchId: "match-80", detailVersion: 2 }],
    nextCursor: { beginTs: 200, matchId: "match-80" },
  },
  coalesceLiveTimelineRevision(undefined, event({ phase: "detail" })),
);
assert.equal(detailRefresh.matches.length, 120, "a detail refresh never truncates loaded pages");
assert.equal(detailRefresh.matches.find((row) => row.matchId === "match-80")?.detailVersion, 2, "detail refresh replaces the affected row");
assert.equal(detailRefresh.matches.at(-1)?.matchId, "match-119", "detail refresh preserves the pagination tail");
assert.deepEqual(detailRefresh.nextCursor, { beginTs: 100, matchId: "old-tail" }, "detail refresh preserves the loaded tail cursor");
assert.equal(detailRefresh.resetViewport, false, "detail refresh keeps the current viewport");

const dirtyDraft = matchFavoriteDraftReducer(createMatchFavoriteDraft({ note: "saved" }), { type: "edit", note: "unsaved draft" });
assert.equal(
  shouldResetMatchFavoriteDraft({ previousMatchId: "match-80", matchId: "match-80", favorite: { note: "server update" }, state: dirtyDraft }),
  false,
  "same-match live detail refresh cannot overwrite an unsaved note draft",
);

activeAccountScope = "account-c";
await controller.start();
assert.equal(listeners.size, 1, "a StrictMode-style stop/start cycle leaves exactly one event subscription");
const burstQueries: number[] = [];
const burstUnsubscribe = controller.subscribe((event) => burstQueries.push(event.revision));
for (let revision = 1; revision <= 5; revision += 1) {
  await emit({ accountScope: "account-c", season: 23, revision, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
}
assert.equal(burstQueries.length, 0, "event bursts do not query before the debounce window closes");
flush();
assert.deepEqual(burstQueries, [5], "a five-mode event burst causes one subscriber query with the latest revision");

controller.stop();
await emit({ accountScope: "account-c", season: 23, revision: 6, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
flush();
assert.deepEqual(burstQueries, [5], "events emitted while stopped cannot leak through a stale subscription");
await controller.start();
assert.equal(listeners.size, 1, "restarting after a stopped interval installs one fresh subscription");
await emit({ accountScope: "account-c", season: 23, revision: 6, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
flush();
assert.deepEqual(burstQueries, [5, 6], "the restarted subscription resumes from the next unseen revision exactly once");

const visibleLatencySamples: number[] = [];
for (let revision = 6; revision <= 45; revision += 1) {
  const started = performance.now();
  await emit({ accountScope: "account-c", season: 23, revision, phase: "list", newMatches: 1, pendingDetails: 0, status: "running" });
  flush();
  const refreshed = reconcileLiveTimelinePage(
    { matches: [{ matchId: "older" }], nextCursor: null },
    { matches: [{ matchId: `new-${revision}` }], nextCursor: null },
    coalesceLiveTimelineRevision(undefined, event({ accountScope: "account-c", revision, phase: "list", newMatches: 1 })),
  );
  assert.equal(refreshed.matches[0]?.matchId, `new-${revision}`);
  visibleLatencySamples.push(performance.now() - started);
}
visibleLatencySamples.sort((a, b) => a - b);
const visibleP95 = visibleLatencySamples[Math.ceil(visibleLatencySamples.length * 0.95) - 1]!;
assert.ok(visibleP95 <= 150, `event-to-visible P95 ${visibleP95.toFixed(2)}ms exceeded 150ms`);
burstUnsubscribe();
controller.stop();

const strictModeListeners = new Set<(event: SeasonDataRevisionEvent) => void>();
const pendingListenStarts: Array<() => void> = [];
const strictModeTimers: Array<() => void> = [];
const strictModeController = createLiveMatchTimelineController({
  listen: (listener) => new Promise((resolve) => {
    pendingListenStarts.push(() => {
      strictModeListeners.add(listener);
      resolve(() => strictModeListeners.delete(listener));
    });
  }),
  getActiveAccountScope: async () => "account-strict",
  schedule: (callback) => { strictModeTimers.push(callback); return callback; },
  cancel: (timer) => {
    const index = strictModeTimers.indexOf(timer as () => void);
    if (index >= 0) strictModeTimers.splice(index, 1);
  },
});
const strictModeNotifications: LiveTimelineInvalidation[] = [];
strictModeController.subscribe((invalidation) => strictModeNotifications.push(invalidation));
const discardedStart = strictModeController.start();
strictModeController.stop();
const retainedStart = strictModeController.start();
assert.equal(pendingListenStarts.length, 2, "StrictMode may restart while the first asynchronous listener is still opening");
pendingListenStarts.shift()!();
await discardedStart;
assert.equal(strictModeListeners.size, 0, "a listener opened by a discarded start is released immediately");
pendingListenStarts.shift()!();
await retainedStart;
assert.equal(strictModeListeners.size, 1, "the retained StrictMode start owns exactly one listener");
strictModeListeners.forEach((listener) => listener(event({
  accountScope: "account-strict",
  revision: 1,
  phase: "list",
  newMatches: 1,
})));
await Promise.resolve();
while (strictModeTimers.length) strictModeTimers.shift()!();
assert.equal(strictModeNotifications.length, 1, "the retained listener delivers the first event after a StrictMode restart");
strictModeController.stop();
console.log("live match timeline tests passed");
