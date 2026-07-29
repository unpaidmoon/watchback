import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { PlayerQueryForm } from "../src/features/player-query/PlayerQueryForm";
import { formatQueryDuration, PlayerProfileCard } from "../src/features/player-query/PlayerProfileCard";

const form = renderToStaticMarkup(<PlayerQueryForm disabled={false} searching={true} onSubmit={() => undefined} />); assert.match(form, /正在查询/); assert.doesNotMatch(form, /取消/);
assert.doesNotMatch(form, /支持半角|全角/);
const textWhileImagesPending = renderToStaticMarkup(<PlayerProfileCard card={{ battleTag: "Player#123", level: 123, gameTimeSeconds: 90, title: "先锋", gameAction: null }} images={null} battleTag="Player#123" />);
assert.match(textWhileImagesPending, /Player#123/);
assert.match(textWhileImagesPending, /123/);
assert.match(textWhileImagesPending, /先锋/);
assert.doesNotMatch(textWhileImagesPending, /data:image\//, "card text renders while the independent image resource is pending");
const cardWithImages = renderToStaticMarkup(
  <PlayerProfileCard
    card={{ battleTag: "Player#123", level: 123, gameTimeSeconds: 90, title: "先锋", gameAction: null }}
    images={{
      icon: { mimeType: "image/png", dataBase64: "AQID" },
      titleIcon: { mimeType: "image/png", dataBase64: "BAUG" },
    }}
    battleTag="Player#123"
  />,
);
assert.match(cardWithImages, /src="data:image\/png;base64,AQID"/, "query profile renders the icon returned by cardImages");
assert.doesNotMatch(cardWithImages, /src="data:image\/png;base64,BAUG"/, "query profile intentionally hides titleIcon");
assert.equal(formatQueryDuration(3_756_060), "1043小时");
const page = readFileSync(new URL("../src/pages/PlayerQueryPage.tsx", import.meta.url), "utf8");
const profileCard = readFileSync(new URL("../src/features/player-query/PlayerProfileCard.tsx", import.meta.url), "utf8");
assert.doesNotMatch(page, /<PageHeader/, "query page starts directly with the profile and search region");
assert.match(page, /查询玩家/); assert.match(page, /请先登录本地账号/); assert.match(page, /RoleRankOverview/); assert.match(page, /matches\.data\?\.\[0\]/); assert.doesNotMatch(page, /localStorage|sessionStorage|loadPlayerQueryMatchDetail|page:\s*2/);
assert.match(page, /query\.selectMode\(modeOrder\[0\]\)/, "the persisted leftmost mode becomes selected even when the query page keeps a live session");
assert.match(page, /modeOrderSynchronized/);
assert.match(page, /if \(!modeOrderSynchronized \|\| !queryIntent/, "startup auto-query waits until the persisted mode order is applied");
assert.match(page, /disabled=\{!modeOrderSynchronized \|\|/, "manual startup query also waits for the persisted mode order");
assert.match(page, /endReached:\s*query\.state\.matchesEndReached/, "handoff preserves the backend raw-page end marker");
assert.equal((page.match(/data-query-region=/g) ?? []).length, 3, "query home has exactly three product regions");
assert.match(page, /data-query-region="identity"[\s\S]*PlayerProfileCard[\s\S]*PlayerQueryForm[\s\S]*PageSeasonSelect/, "profile stays left of the combined search and season controls");
assert.doesNotMatch(page, /SectionHeading title="资料卡"/);
assert.match(profileCard, /ProfileSummaryCard/, "query reuses the command-center profile presentation module");
assert.match(profileCard, /showTitleIcon=\{false\}/, "query disables only the titleIcon region on the shared profile card");
assert.doesNotMatch(profileCard, /avatarAction=/, "query avatar is display-only and cannot refresh profile data");
assert.doesNotMatch(profileCard, /getPlayerGameAction|invoke\(/, "query game-action dialog uses data fetched with the query");
assert.match(page, /playerQueryRanksPanel phase6RoleReport reviewRoleHeroPanel/, "rank region reuses the report container visual");
assert.match(page, /openCompetitive=\{mapRanks\(ranks\.openCompetitive \?\? \[\]\)\}/, "query presents the open competitive rank group independently");
assert.match(page, /open:\s*"开放"/, "query maps the single open rank label consistently with review");
assert.match(page, /playerQueryColumnPanel playerQueryMatchesColumn/);
assert.match(page, /playerQueryColumnPanel playerQueryHeroesColumn/);
assert.doesNotMatch(page, /endReached:\s*query\.state\.matches\.data\.length|endReached:\s*matches\.length/, "handoff must not infer endReached from the displayed list length");
const css = readFileSync(new URL("../src/styles/features/player-query.css", import.meta.url), "utf8"); assert.match(css, /grid-template-columns:minmax\(0,1fr\) minmax\(0,1fr\)/); assert.match(css, /max-width:1100px/); assert.doesNotMatch(css, /playerQueryMatchRows[^}]*overflow-y:\s*(auto|scroll)/);
assert.match(css, /max-width:\s*var\(--phase6-content-max\)/);
assert.match(css, /reviewCompetitiveRankGrid[^}]*repeat\(4/);
assert.match(css, /playerQueryIdentityLayout[^}]*grid-template-columns:\s*minmax\(450px, 1fr\) minmax\(0, 1\.55fr\)/, "query places the command-center-sized profile card left of a wider search area");
assert.doesNotMatch(css, /playerQueryProfileArea[^}]*border-bottom/, "the side-by-side profile is not separated as a full-width top row");
assert.match(css, /@media \(max-width:1100px\)[\s\S]*playerQueryIdentityLayout,[\s\S]*grid-template-columns:\s*1fr/, "identity and search stack on narrower windows");
assert.match(css, /pageSeasonSelect label[\s\S]*clip:/, "season remains accessible without a visible mini heading");
assert.doesNotMatch(css, /playerQueryForm > label/);
assert.match(form, /aria-label="BattleTag"/, "the input keeps a stable accessible name without a hidden label");
assert.doesNotMatch(css, /--accent|--surface,|--text-muted|--danger|--border,/);
const hook = readFileSync(new URL("../src/features/player-query/usePlayerQuerySession.ts", import.meta.url), "utf8");
assert.doesNotMatch(page, /<PageSeasonSelect[^>]*disabled=/, "season selection stays available before a query starts");
assert.match(hook, /sessionId \? changePlayerQuerySeason\(current, season\) : \{ \.\.\.current, season \}/, "an idle season change is kept without loading session resources");
assert.match(hook, /loadPlayerQueryModeHeroes/); assert.match(hook, /void loadMatches[\s\S]*void loadHeroes/); assert.doesNotMatch(hook, /data\.heroes/);
assert.match(hook, /void loadCard\([\s\S]*void loadCardImages\(/, "card text and images start independently without awaiting either resource");
console.log("player-query page tests passed");
