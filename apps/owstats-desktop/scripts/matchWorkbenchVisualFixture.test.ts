import assert from "node:assert/strict";
import { renderMatchWorkbenchFixture, type MatchWorkbenchFixtureState } from "./matchWorkbenchVisualFixture";

const states: MatchWorkbenchFixtureState[] = ["wide-normal", "wide-arena", "narrow-list", "narrow-detail"];

function extractMatchCard(html: string, matchNumber: number): string {
  const matchId = `fixture-match-${matchNumber}`;
  const card = html.match(new RegExp(`<button\\b[^>]*data-match-id="${matchId}"[^>]*>[\\s\\S]*?</button>`));
  assert.ok(card, `未在视觉样本 HTML 中找到对局卡片 ${matchId}`);
  return card[0];
}

for (const state of states) {
  const html = renderMatchWorkbenchFixture(state);
  assert.match(html, /对局记录/);
  assert.doesNotMatch(html, /token|customerToken|authorization|credential|bnetId/i);
  assert.match(html, new RegExp(`data-fixture-state="${state}"`));
}

assert.match(renderMatchWorkbenchFixture("wide-normal"), /aria-label="普通对局详情"/);
assert.match(renderMatchWorkbenchFixture("wide-arena"), /aria-label="角斗对局详情"/);
assert.match(renderMatchWorkbenchFixture("narrow-list"), /data-fixture-state="narrow-list"/);
assert.match(renderMatchWorkbenchFixture("narrow-detail"), /data-fixture-state="narrow-detail"/);
assert.match(
  renderMatchWorkbenchFixture("narrow-list"),
  /@media\s*\(max-width:\s*1099px\)\s*\{[^}]*html,\s*body\s*\{\s*min-width:\s*0;\s*\}/s,
  "anonymous narrow fixtures should opt out of the desktop shell minimum width",
);

const wideNormal = renderMatchWorkbenchFixture("wide-normal");
const wideNormalBody = wideNormal.slice(wideNormal.indexOf("<body"));
assert.match(wideNormal, /预设快速模式长名称示例/);
assert.match(wideNormal, /1\.6w\/2,410\/1\.2w/);
assert.match(wideNormal, /第二波团战站位值得复盘，注意先处理侧翼。/);
assert.ok(
  wideNormalBody.lastIndexOf("matchTeamTable") < wideNormalBody.indexOf("matchFavoriteEditor"),
  "normal detail favorite editor should render after both detail tables",
);
assert.match(
  renderMatchWorkbenchFixture("wide-arena"),
  /aria-label="角斗对局详情"[\s\S]*class="matchFavoriteEditor is-favorite"/,
  "arena summary should render the deterministic favorite editor",
);
assert.match(
  renderMatchWorkbenchFixture("wide-arena"),
  /回合-1/,
  "arena fixture should retain a round selector for verifying that round views omit the editor",
);
assert.match(
  renderMatchWorkbenchFixture("narrow-detail"),
  /data-horizontal-overflow-contract="none"/,
  "narrow detail fixture should explicitly opt into the no-horizontal-overflow contract",
);
assert.doesNotMatch(
  wideNormalBody,
  /(?:BattleTag|displayName|accountId|playerId|真实备注|真实对局)/i,
  "visual fixture should contain synthetic labels and notes only",
);

const firstCard = extractMatchCard(wideNormal, 1);
assert.match(firstCard, /(?:title|aria-label)="伤害最高"/);
assert.match(firstCard, /(?:title|aria-label)="治疗最高"/);
assert.match(firstCard, /class="matchListFavoriteMark"/);

assert.equal(firstCard.match(/phase6AwardMedal/g)?.length, 4, "the widest anonymous card should render four awards");

const sixthCard = extractMatchCard(wideNormal, 6);
assert.match(sixthCard, /phase6PerformanceBadges empty/);
assert.doesNotMatch(sixthCard, /primaryTag/);

const seventhCard = extractMatchCard(wideNormal, 7);
assert.match(seventhCard, /未知地图/);
assert.match(seventhCard, /<span class="phase6AssetFallback [^"]*matchListCardHeaderMap" aria-hidden="true"><\/span>/);
assert.doesNotMatch(seventhCard, /<img[^>]*(?:fixture-missing-map|matchListCardHeaderMap)[^>]*>/);
assert.doesNotMatch(seventhCard, /src="[^"]*fixture-missing-map[^"]*"/);
