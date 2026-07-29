import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  PLAYER_QUERY_MODE_LABELS,
  PLAYER_QUERY_MODES,
  PLAYER_QUERY_SEASONS,
  type PlayerQueryCard,
  type PlayerQueryMatchRow,
} from "../src/features/player-query/playerQueryTypes";

assert.deepEqual(PLAYER_QUERY_SEASONS, [23, 22, 21, 20, 15]);
assert.deepEqual(PLAYER_QUERY_MODES, [
  "LeisurePreset",
  "Leisure6v6",
  "SportPreset",
  "SportOpen",
  "SportFight",
  "LeisureFight",
]);
assert.deepEqual(PLAYER_QUERY_MODE_LABELS, {
  LeisurePreset: "快速预设",
  Leisure6v6: "快速开放",
  SportPreset: "预设竞技",
  SportOpen: "开放竞技",
  SportFight: "角斗竞技",
  LeisureFight: "角斗快速",
});

const card: PlayerQueryCard = {
  battleTag: "示例玩家#1234",
  level: 3,
  gameTimeSeconds: 3600,
  title: null,
  gameAction: null,
};
assert.equal(card.battleTag, "示例玩家#1234");

const row: PlayerQueryMatchRow = {
  matchId: "match-1",
  matchKind: "arena",
  result: "win",
  score: "3-2",
  mapName: null,
  heroKey: "hero-1",
  heroName: "示例英雄",
  kills: 12,
  assists: 8,
  deaths: 3,
  playedAt: "2026-07-26T10:00:00+08:00",
};
assert.equal(row.mapName, null);

const typesSource = readFileSync(new URL("../src/features/player-query/playerQueryTypes.ts", import.meta.url), "utf8");
const apiSource = readFileSync(new URL("../src/features/player-query/playerQueryApi.ts", import.meta.url), "utf8");

assert.doesNotMatch(typesSource, /customerToken|bigdata|\btoken\b|rawJson|raw JSON/i);
assert.doesNotMatch(apiSource, /localStorage|sessionStorage|indexedDB|customerToken|bigdata|\btoken\b/i);
assert.match(typesSource, /PlayerQueryMatchRow\s*=\s*PlayerQueryMatchRowView/);
assert.doesNotMatch(typesSource, /type PlayerQueryCard[\s\S]*?\bname\s*:/);

for (const command of [
  "player_query_start",
  "player_query_card",
  "player_query_card_images",
  "player_query_season",
  "player_query_mode_page",
  "player_query_mode_heroes",
  "player_query_match_detail",
  "player_query_release_more_data",
  "player_query_end",
]) {
  assert.match(apiSource, new RegExp(`\\b${command}\\b`));
}
assert.equal((apiSource.match(/invoke</g) ?? []).length, 9);
assert.doesNotMatch(typesSource, /type PlayerQueryModePage[\s\S]*?heroes\s*:/);
