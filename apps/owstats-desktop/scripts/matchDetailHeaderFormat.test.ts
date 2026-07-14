import assert from "node:assert/strict";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore, resultEnglishLabel } from "../src/shared/matchDetailHeaderFormat";
import type { MatchDetailView } from "../src/shared/types";

const detail = {
  teamScore: 3,
  opponentScore: 0,
  durationSec: 570,
  beginTs: 1781376240,
} as MatchDetailView;

assert.equal(formatFinalScore(detail), "3-0");
assert.equal(formatDetailDuration(detail.durationSec), "9分30秒");
assert.match(formatDetailStartTime(detail.beginTs), /^\d{2}\/\d{2}\/\d{2} \d{2}:\d{2}$/);
assert.equal(formatFinalScore({ ...detail, teamScore: null }), "-");
assert.equal(formatDetailDuration(null), "-");
assert.equal(formatDetailStartTime(null), "-");
assert.equal(resultEnglishLabel("win"), "VICTORY");
assert.equal(resultEnglishLabel("loss"), "DEFEAT");
assert.equal(resultEnglishLabel("draw"), "DRAW");
assert.equal(resultEnglishLabel(null), "UNKNOWN");
assert.equal(resultEnglishLabel("pending"), "UNKNOWN");
