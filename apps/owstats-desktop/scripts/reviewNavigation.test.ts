import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  buildFriendDetailState,
  buildAchievementsHref,
  buildReviewHref,
  buildReviewRankingHref,
  legacyReviewRedirectHref,
  parseFriendDetailState,
  reviewScrollKey,
  scheduleReviewScrollRestore,
} from "../src/features/review/reviewNavigation";

const reviewPageSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
assert.match(reviewPageSource, /if \(!Number\.isFinite\(scrollY\)\) \{ sessionStorage\.removeItem\(reviewScrollKey\(href\)\); return; \}/, "invalid persisted review scroll values must be cleared");

assert.equal(buildReviewHref(23), "/review?season=23");
assert.equal(buildAchievementsHref(23), "/achievements");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=achievements")), "/achievements");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=friends")), "/review?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=unknown")), "/review?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=abc&view=achievements")), "/achievements");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=0&view=friends")), "/review");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=-1&view=overview")), "/review");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23.5&view=maps")), "/review");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23")), null);
assert.equal(buildReviewRankingHref(22), "/ranking?tab=maps&season=22");

const detailState = buildFriendDetailState("private-id", 21, 640);
assert.equal(detailState.bnetId, "private-id");
assert.equal(detailState.season, 21);
assert.deepEqual(detailState.returnTo, {
  href: "/review?season=21",
  scrollKey: "review-scroll:/review?season=21",
  scrollY: 640,
});
assert.equal(reviewScrollKey("/review?season=21"), detailState.returnTo.scrollKey);
assert.equal(detailState.returnTo.href.includes("private-id"), false);
assert.equal(parseFriendDetailState(null), null);
assert.equal(parseFriendDetailState({ ...detailState, season: 0 }), null);
assert.equal(parseFriendDetailState({ ...detailState, returnTo: { ...detailState.returnTo, href: "/matches" } }), null);
assert.equal(parseFriendDetailState({ ...detailState, returnTo: { ...detailState.returnTo, href: "http://[" } }), null);
assert.deepEqual(parseFriendDetailState(detailState), detailState);

const scheduled: Array<() => void> = [];
let maxScrollY = 0;
let scrollY = 0;
let removed = false;
const cancelRestore = scheduleReviewScrollRestore(120, {
  requestFrame(callback) {
    scheduled.push(callback);
    return scheduled.length;
  },
  cancelFrame() {},
  scrollTo(top) {
    scrollY = Math.min(top, maxScrollY);
  },
  readScrollY: () => scrollY,
  readMaxScrollY: () => maxScrollY,
  removeStored: () => {
    removed = true;
  },
});
scheduled.shift()?.();
assert.equal(scrollY, 0, "内容尚未撑开时滚动位置会被浏览器夹到顶部");
assert.equal(removed, false, "内容尚未撑开时不应提前清除恢复记录");
maxScrollY = 400;
scheduled.shift()?.();
assert.equal(scrollY, 120, "异步内容挂载后应恢复原滚动位置");
assert.equal(removed, true, "成功恢复后应清除一次性记录");
cancelRestore();
