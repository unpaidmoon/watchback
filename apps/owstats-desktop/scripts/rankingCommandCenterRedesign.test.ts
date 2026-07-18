import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { phase6NavItems } from "../src/shared/phase6Shell";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");
const rankingCss = readFileSync(new URL("../src/styles/features/ranking.css", import.meta.url), "utf8");

assert.equal(phase6NavItems.map((item) => item.label).join(","), "对局,战报,成就,排行,生涯 MMR,指挥台");
assert.match(appSource, /path="\/" element=\{<Navigate to="\/matches" replace \/>\}/);
assert.match(rankingSource, /<PageHeader/);
assert.match(rankingSource, /<PageSeasonSelect/);
assert.match(rankingSource, /<PageLoadingState/);
assert.match(rankingSource, /<PageErrorState/);
assert.match(rankingSource, /<PageEmptyState/);

for (const contract of [
  /getRankings\(selectedSeason\)/,
  /const heroSortKeys: RankingSortKey\[\] = \["matches", "winRate", "playTimeHours", "kda"\]/,
  /const mapSortKeys: RankingSortKey\[\] = \["matches", "winRate", "playTimeHours", "kda"\]/,
  /sortRankingRows\(filtered, sort\)/,
  /ranking\?\.availableSeasons/,
  /nextParams\.set\("season", String\(nextSeason\)\)/,
]) assert.match(rankingSource, contract);

assert.match(rankingSource, /title="英雄与地图排行"/);
assert.match(rankingSource, /<PageHeader[\s\S]*actions=\{\([\s\S]*<PageSeasonSelect/);
assert.match(rankingSource, /className="rankingToolbar phase6RankingToolbar"[\s\S]*phase6SegmentedControl[\s\S]*phase6RankingSearch[\s\S]*phase6RankingCount/);
assert.doesNotMatch(rankingSource, /className="rankingToolbar phase6RankingToolbar"[\s\S]*phase8SeasonSelect[\s\S]*<\/div>/);
assert.match(
  rankingCss,
  /\.phase6RankingTable\s*\{[^}]*border:\s*1px solid var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);[^}]*box-shadow:\s*none;/s,
  "排行表格应作为唯一一级玻璃表面，不叠加阴影",
);
assert.match(rankingCss, /\.phase6RankingHead\s*\{[^}]*background:\s*var\(--glass-secondary\)/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*min-height:\s*52px/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*box-shadow:\s*none/s);
assert.match(rankingCss, /\.phase6SegmentedControl button\.active\s*\{[^}]*background:\s*var\(--action-primary\)/s);
assert.match(rankingCss, /\.phase6RankingHead button\.active\s*\{[^}]*color:\s*var\(--text-primary\)/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*background:\s*transparent;[^}]*font-weight:\s*400;/s);

assert.match(
  rankingSource,
  /className="segmentedControl phase6SegmentedControl"\s+role="group"\s+aria-label="排行类型"/,
  "英雄与地图切换应作为有名称的分段按钮组暴露",
);
assert.match(rankingSource, /aria-pressed=\{tab === "heroes"\}/, "英雄排行按钮应暴露按下状态");
assert.match(rankingSource, /aria-pressed=\{tab === "maps"\}/, "地图排行按钮应暴露按下状态");
assert.match(
  rankingSource,
  /className=\{`rankingTable phase6RankingTable \$\{tab\}`\}[\s\S]*?role="table"[\s\S]*?aria-rowcount=\{rows\.length \+ 1\}/,
  "排行网格应暴露表格角色和完整行数",
);
assert.match(rankingSource, /className="rankingHead phase6RankingHead" role="row"/, "排行表头应暴露行角色");
assert.match(rankingSource, /className="rankingLine phase6RankingLine"[\s\S]*?role="row"/, "排行数据应暴露行角色");
assert.match(rankingSource, /role="columnheader"/, "排行表头单元格应暴露列标题角色");
assert.match(rankingSource, /role="cell"/, "排行数据单元格应暴露表格单元格角色");
assert.match(
  rankingSource,
  /role="columnheader"[\s\S]*?aria-sort=\{active[\s\S]*?"descending"[\s\S]*?"ascending"[\s\S]*?: undefined\}/,
  "当前排序列应在列标题上暴露升序或降序",
);
assert.match(
  rankingSource,
  /aria-label=\{active[\s\S]*?当前[\s\S]*?点击切换为/,
  "当前排序按钮的可访问名称应说明方向和下一步操作",
);
