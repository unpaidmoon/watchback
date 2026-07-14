import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { phase6NavItems } from "../src/shared/phase6Shell";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");
const rankingCss = readFileSync(new URL("../src/styles/features/ranking.css", import.meta.url), "utf8");

assert.equal(phase6NavItems.map((item) => item.label).join(","), "对局,战报,生涯 MMR,排行,指挥台");
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
assert.match(rankingCss, /\.phase6RankingHead\s*\{[^}]*background:\s*var\(--surface-primary\)/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*min-height:\s*52px/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*box-shadow:\s*none/s);
assert.match(rankingCss, /\.phase6SegmentedControl button\.active\s*\{[^}]*background:\s*var\(--action-primary\)/s);
assert.match(rankingCss, /\.phase6RankingHead button\.active\s*\{[^}]*color:\s*var\(--data-primary\)/s);
