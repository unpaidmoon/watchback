import assert from "node:assert/strict";
import {
  buildFriendDetailState,
  buildReviewHref,
  buildReviewRankingHref,
  parseFriendDetailState,
  parseReviewView,
  reviewScrollKey,
  scheduleReviewScrollRestore,
} from "../src/features/review/reviewNavigation";

assert.equal(parseReviewView(null), "overview");
assert.equal(parseReviewView("roles"), "roles");
assert.equal(parseReviewView("friends"), "friends");
assert.equal(parseReviewView("maps"), "maps");
assert.equal(parseReviewView("achievements"), "achievements");
assert.equal(parseReviewView("invalid"), "overview");
assert.equal(buildReviewHref(23, "friends"), "/review?season=23&view=friends");
assert.equal(buildReviewRankingHref(22), "/ranking?tab=maps&season=22");

const detailState = buildFriendDetailState("private-id", 21, "maps", 640);
assert.equal(detailState.bnetId, "private-id");
assert.equal(detailState.season, 21);
assert.deepEqual(detailState.returnTo, {
  href: "/review?season=21&view=maps",
  scrollKey: "review-scroll:/review?season=21&view=maps",
  scrollY: 640,
});
assert.equal(reviewScrollKey("/review?season=21&view=maps"), detailState.returnTo.scrollKey);
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
