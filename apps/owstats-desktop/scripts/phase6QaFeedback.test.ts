import assert from "node:assert/strict";
import {
  formatQaFeedbackText,
  isQaEnabledFromSearch,
  preserveQaParam,
  qaPageForPath,
  qaRegionsForPath,
} from "../src/shared/qaFeedback";

assert.equal(isQaEnabledFromSearch("?qa=1"), true);
assert.equal(isQaEnabledFromSearch("?season=22&qa=true"), true);
assert.equal(isQaEnabledFromSearch("?qa=0"), false);
assert.equal(isQaEnabledFromSearch(""), false);
assert.equal(preserveQaParam(new URLSearchParams("qa=1&season=22"), new URLSearchParams("hero=ana")).toString(), "hero=ana&qa=1");
assert.equal(preserveQaParam(new URLSearchParams("qa=0"), new URLSearchParams("hero=ana")).toString(), "hero=ana");

assert.equal(qaPageForPath("/review").label, "复盘");
assert.notEqual(qaPageForPath("/matches/demo-match").label, "对局详情");
assert.equal(qaPageForPath("/analytics/friends/123").label, "队友详情");

assert.deepEqual(
  qaRegionsForPath("/review").map((region) => region.id),
  ["NAV-CAPSULE", "REV-RANKS", "FRD-BOARDS", "FRD-LINEUP", "FRD-MAPS"],
);
assert.deepEqual(
  qaRegionsForPath("/ranking").map((region) => region.id),
  ["NAV-CAPSULE", "RANK-TOOLS", "RANK-TABLE"],
);

assert.equal(
  formatQaFeedbackText({ pageLabel: "复盘", regionId: "REV-RANKS", regionLabel: "职责段位总览" }),
  ["页面：复盘", "区域：REV-RANKS（职责段位总览）", "问题：", "期望：", "优先级：P1"].join("\n"),
);
