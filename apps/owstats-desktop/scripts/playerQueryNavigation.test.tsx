import assert from "node:assert/strict";
import type { ReactElement } from "react";
import { PlayerQueryMatchList } from "../src/features/player-query/PlayerQueryMatchList";
import type { PlayerQueryMatchRow } from "../src/features/player-query/playerQueryTypes";
import { phase6NavItems } from "../src/shared/phase6Shell";

assert.deepEqual(
  phase6NavItems.slice(0, 3).map(({ label, path }) => ({ label, path })),
  [
    { label: "对局", path: "/matches" },
    { label: "查询", path: "/player-query" },
    { label: "战报", path: "/review" },
  ],
);
assert.equal(new Set(phase6NavItems.map((item) => item.path)).size, phase6NavItems.length, "主导航路径不得重复");
assert.equal(phase6NavItems.find((item) => item.path === "/player-query")?.title, "即时查询玩家数据");

const match: PlayerQueryMatchRow = {
  matchId: "match-1",
  matchKind: "normal",
  result: "win",
  score: "3-1",
  mapName: "国王大道",
  heroName: "安娜",
  playedAt: "2026-01-01T00:00:00+08:00",
  kills: 10,
  assists: 8,
  deaths: 2,
};
const selected: string[] = [];
let openMoreCount = 0;
const list = PlayerQueryMatchList({
  matches: [match],
  onOpenMatch: (row) => selected.push(row.matchId),
  onOpenMore: () => { openMoreCount += 1; },
});
const [rows, moreButton] = list.props.children as ReactElement[];
const rowChildren = rows.props.children as ReactElement | ReactElement[];
const row = Array.isArray(rowChildren) ? rowChildren[0] : rowChildren;
(row.props as { onSelect: () => void }).onSelect();
(moreButton.props as { onClick: () => void }).onClick();
assert.deepEqual(selected, ["match-1"], "选择预览行时导航使用该行的 matchId");
assert.equal(openMoreCount, 1, "更多对局动作只触发一次导航回调");

console.log("player query navigation tests passed");
