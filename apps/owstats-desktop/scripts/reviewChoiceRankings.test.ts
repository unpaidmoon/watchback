import assert from "node:assert/strict";
import fs from "node:fs";

const types = fs.readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");

assert.match(types, /choiceRankings: ChoiceRankings/);
assert.match(types, /normalPerks: ChoiceRankingEntry\[\]/);
assert.match(types, /stadiumTraits: ChoiceRankingEntry\[\]/);
assert.match(types, /stadiumMods: ChoiceRankingEntry\[\]/);

const review = fs.readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const panel = fs.readFileSync(new URL("../src/features/review/ChoiceRankingsPanel.tsx", import.meta.url), "utf8");
assert.match(review, /<ChoiceRankingsPanel[\s\S]*?<MapAnalysisModules/);
assert.match(panel, /普通对局 · 威能/);
assert.match(panel, /角斗 · 异能/);
assert.match(panel, /角斗 · 装备/);
assert.match(panel, /至少选取.*按 95% 置信统计排序/);
assert.doesNotMatch(panel, /置信下限/);
assert.match(panel, /resolvePerkInfo/);
assert.match(panel, /resolveArenaModTraitInfo/);
assert.match(panel, /phase6ChoiceRankingIcon--perk/);
assert.match(panel, /样本不足，至少需要/);

const styles = fs.readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");
assert.match(styles, /\.phase6ChoiceRankingsPanel/);
assert.match(styles, /\.phase6ChoiceRankingsGrid[\s\S]*grid-template-columns:\s*repeat\(3/);
assert.match(styles, /\.phase6ChoiceRankingColumn \+ \.phase6ChoiceRankingColumn/);
assert.match(styles, /\.phase6ChoiceRankingIcon[\s\S]*32px/);
assert.match(
  styles,
  /\.phase6ChoiceRankingIcon--perk\s*\{[\s\S]*background:\s*#fff;[\s\S]*border:\s*1px solid var\(--glass-border\);/,
);
assert.match(styles, /@media \(max-width:\s*900px\)[\s\S]*\.phase6ChoiceRankingsGrid/);
