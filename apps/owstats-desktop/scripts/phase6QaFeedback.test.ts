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
assert.equal(qaPageForPath("/matches/demo-match").label, "对局详情");
assert.equal(qaPageForPath("/analytics/friends/123").label, "队友详情");

assert.deepEqual(
  qaRegionsForPath("/review").map((region) => region.id),
  ["NAV-CAPSULE", "REV-HEADER", "REV-SUMMARY", "REV-EVIDENCE", "REV-MAP-CARD"],
);

assert.equal(
  formatQaFeedbackText({ pageLabel: "复盘", regionId: "REV-EVIDENCE", regionLabel: "复盘证据" }),
  ["页面：复盘", "区域：REV-EVIDENCE（复盘证据）", "问题：", "期望：", "优先级：P1"].join("\n"),
);
