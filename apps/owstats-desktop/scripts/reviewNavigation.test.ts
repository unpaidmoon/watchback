import assert from "node:assert/strict";
import {
  buildFriendDetailState,
  buildReviewHref,
  buildReviewRankingHref,
  parseFriendDetailState,
  parseReviewView,
  reviewScrollKey,
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
