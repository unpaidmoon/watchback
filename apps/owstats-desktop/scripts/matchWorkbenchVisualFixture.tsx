import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { extname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { FilterBar } from "../src/features/matches/FilterBar";
import { MatchDetailPane } from "../src/features/matches/MatchDetailPane";
import { MatchTimeline } from "../src/features/matches/MatchTimeline";
import type { MatchFavoriteController } from "../src/features/matches/useMatchFavoriteEditor";
import { PageHeader, PageSeasonSelect, StatusPill } from "../src/shared/page-shell";
import type { ArenaDetailSlice, FilterOptions, MatchDetailView, MatchFilters, MatchSummary, PlayerRow } from "../src/shared/types";

export type MatchWorkbenchFixtureState = "wide-normal" | "wide-arena" | "narrow-list" | "narrow-detail";

const fixtureStyles = [
  "../src/styles/tokens.css",
  "../src/styles/shared.css",
  "../src/styles/base.css",
  "../src/styles/shell.css",
  "../src/styles/page-shell.css",
  "../src/styles/features/matches.css",
]
  .map((path) => readFileSync(new URL(path, import.meta.url), "utf8"))
  .join("\n")
  .replaceAll("phase6ProfileAchievementToken", "phase6ProfileAchievementMark");

const fixtureOnlyStyles = `
html { min-height: 100%; background: var(--app-outside); }
body { min-height: 100%; margin: 0; padding: 8px; }
.fixtureBrandMark { width: 40px; height: 40px; display: grid; place-items: center; border-radius: 50%; background: var(--action-primary); color: var(--action-on-primary); font-weight: 700; }
.fixtureShellStatus { white-space: nowrap; }
@media (max-width: 1099px) {
  html, body { min-width: 0; }
  body { padding: 0; }
  .appFrame { min-height: 100vh; border-radius: 0; }
}
`;

const filters: MatchFilters = { season: 22, mode: "SportPreset", hero: "sigma", map: "shambali" };
const options: FilterOptions = {
  seasons: [21, 22, 23],
  heroes: [
    { id: "sigma", label: "西格玛" },
    { id: "ana", label: "安娜" },
  ],
  maps: [
    { id: "shambali", label: "香巴里寺院" },
    { id: "oasis", label: "绿洲城 - 花园" },
  ],
  results: ["win", "loss", "draw"],
  modes: ["SportPreset", "LeisureFight"],
};

const players = Array.from({ length: 12 }, (_, index) => createPlayer(index));
const matches = Array.from({ length: 7 }, (_, index) => createMatch(index));
const anonymousFavoriteNote = "第二波团战站位值得复盘，注意先处理侧翼。";
const anonymousNormalDetail = createNormalDetail();
const anonymousArenaDetail = createArenaDetail();
const favoriteController: MatchFavoriteController = {
  savedNote: anonymousFavoriteNote,
  draftNote: anonymousFavoriteNote,
  isFavorite: true,
  busy: false,
  error: null,
  saved: false,
  dirty: false,
  count: anonymousFavoriteNote.length,
  canSave: false,
  edit: () => undefined,
  favoriteMatch: async () => undefined,
  saveNote: async () => undefined,
  unfavoriteMatch: async () => undefined,
  confirmDiscard: () => true,
};

export function renderMatchWorkbenchFixture(state: MatchWorkbenchFixtureState): string {
  const arena = state === "wide-arena";
  const detail = arena ? anonymousArenaDetail : anonymousNormalDetail;
  const detailOpen = state === "narrow-detail";
  const markup = inlineLocalAssets(renderToStaticMarkup(
    <FixtureApp state={state} detail={detail} detailOpen={detailOpen} />,
  ));
  const overflowContract = state === "narrow-detail" ? ' data-horizontal-overflow-contract="none"' : "";
  return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><style>${fixtureStyles}\n${fixtureOnlyStyles}</style></head><body data-fixture-state="${state}"${overflowContract}>${markup}</body></html>`;
}

function FixtureApp({ state, detail, detailOpen }: { state: MatchWorkbenchFixtureState; detail: MatchDetailView; detailOpen: boolean }) {
  return (
    <main className="appFrame appShell phase6ProductShell" data-fixture-app={state}>
      <header className="appTopBar phase6ProductTopBar">
        <div className="appBrand phase6ProductBrand">
          <span className="fixtureBrandMark" aria-hidden="true">WB</span>
          <div>
            <strong className="appBrandName">Watchback</strong>
            <p>守望战绩工具</p>
          </div>
        </div>
        <nav className="appNav phase6ProductNav" aria-label="一级导航">
          {[
            ["对局", true],
            ["战报", false],
            ["排行", false],
            ["指挥台", false],
          ].map(([label, active]) => (
            <a className={`appNavLink ${active ? "active" : ""}`} aria-current={active ? "page" : undefined} href={`#${label}`} key={String(label)}>{label}</a>
          ))}
        </nav>
        <div className="appShellStatus fixtureShellStatus">
          <StatusPill tone="good">本地数据已就绪</StatusPill>
        </div>
      </header>
      <div className="appCanvas phase6ProductSurface">
        <div className="matchesPage">
          <PageHeader
            title="对局记录"
            description="7 场匿名示例对局"
            actions={(
              <PageSeasonSelect
                value={22}
                seasons={[21, 22, 23]}
                includeAll
                allLabel="全部赛季"
                onChange={() => undefined}
              />
            )}
          />
          <div className="matchesPageControls">
            <FilterBar filters={filters} options={options} onChange={() => undefined} />
          </div>
          <section className={`matchesWorkbench workspace listWorkspace phase6ListWorkspace phase6MatchWorkbench ${detailOpen ? "detailOpen" : ""}`}>
            <div className="matchListPane phase6WorkbenchList">
              <MatchTimeline matches={matches} selectedMatchId={matches[0].matchId} onSelect={() => undefined} />
            </div>
            <MatchDetailPane
              detail={detail}
              loading={false}
              error={null}
              matchId={detail.matchId}
              favoriteController={favoriteControllerForState(state)}
              onBack={() => undefined}
            />
          </section>
        </div>
      </div>
    </main>
  );
}

function createMatch(index: number): MatchSummary {
  const result = (["win", "win", "loss", "draw"] as const)[index % 4];
  const arena = index === 2;
  const missingMap = index === 6;
  return {
    matchId: `fixture-match-${index + 1}`,
    season: 22,
    bucket: "preset",
    matchKind: arena ? "fight" : "preset",
    gameMode: arena ? "LeisureFight" : "SportPreset",
    modeLabel: index === 0 ? "预设快速模式长名称示例" : arena ? "角斗竞技" : "竞技比赛",
    result,
    resultLabel: result === "win" ? "胜利" : result === "loss" ? "失败" : "平局",
    mapGuid: missingMap ? "fixture-missing-map" : arena ? "oasis" : "shambali-monastery",
    mapPreviewGuid: missingMap ? "fixture-missing-map" : arena ? "oasis" : "shambali-monastery",
    mapName: missingMap ? "未知地图" : arena ? "绿洲城 - 花园" : "香巴里寺院",
    heroGuid: index % 2 === 0 ? "sigma" : "ana",
    heroName: index % 2 === 0 ? "西格玛" : "安娜",
    beginTs: 1783958400 - index * 3600,
    beginLabel: `07/14 ${String(14 - index).padStart(2, "0")}:20`,
    teamScore: result === "loss" ? 1 : 3,
    opponentScore: result === "win" ? 1 : result === "draw" ? 2 : 3,
    kills: 28 - index,
    assists: 14 + index,
    deaths: 5 + index,
    damage: 15842 - index * 631,
    healing: 2410 + index * 337,
    mitigation: 12380 - index * 405,
    durationSec: 780 + index * 17,
    evaluation: index === 5 ? null : {
      grade: index % 3 === 0 ? "S" : "A",
      primaryTag: index % 3 === 0 ? "尽力局" : "伯仲局",
      evidenceTags: [],
      confidence: 0.86,
    },
    teamAwards: index === 0 ? [
      { code: "damage", label: "伤害最高", tone: "gold" },
      { code: "healing", label: "治疗最高", tone: "gold" },
      { code: "kill", label: "击杀最高", tone: "gold" },
      { code: "mitigation", label: "抵挡最高", tone: "gold" },
    ] : [],
    isFavorite: index === 0,
  };
}

function favoriteControllerForState(state: MatchWorkbenchFixtureState): MatchFavoriteController {
  if (state !== "narrow-detail") return favoriteController;
  const note = "复".repeat(500);
  return {
    ...favoriteController,
    savedNote: anonymousFavoriteNote,
    draftNote: note,
    saved: false,
    error: "匿名保存错误，请稍后重试。",
    dirty: true,
    count: note.length,
    canSave: true,
  };
}

function createPlayer(index: number): PlayerRow {
  const teammate = index < 6;
  return {
    side: teammate ? "teammate" : "enemy",
    name: `示例玩家${String.fromCharCode(65 + index)}`,
    bnetId: "",
    rankLabel: null,
    rankKey: null,
    heroGuid: index % 2 === 0 ? "sigma" : "ana",
    heroName: index % 2 === 0 ? "西格玛" : "安娜",
    kills: 24 - (index % 5),
    assists: 10 + (index % 6),
    deaths: 4 + (index % 4),
    damage: 14820 - index * 317,
    healing: 1800 + index * 421,
    mitigation: 11240 - index * 289,
    finalHits: 8 - (index % 4),
    targetTimeSec: 42 + index,
    healingTaken: 6200 + index * 133,
    damageTaken: 9800 + index * 211,
    worth: 7200 + index * 100,
    endWorth: 8400 + index * 100,
    worthMax: index === 0,
    modGuids: [],
    traitGuids: [],
    hasSourceMaxFlags: true,
    killMax: index === 0,
    heroDamageMax: index === 0,
    cureMax: index === 1,
    resistDamageMax: index === 0,
    perks: [],
    heroStats: [],
  };
}

function createNormalDetail(): MatchDetailView {
  return {
    matchId: "fixture-normal",
    gameMode: "SportPreset",
    modeLabel: "竞技比赛",
    result: "win",
    resultLabel: "胜利",
    mapGuid: "shambali-monastery",
    mapName: "香巴里寺院",
    heroGuid: "sigma",
    heroName: "西格玛",
    beginTs: 1783958400,
    beginLabel: "26/07/14 00:00",
    teamScore: 3,
    opponentScore: 1,
    durationSec: 923,
    teammates: players.slice(0, 6),
    enemies: players.slice(6, 12),
    arenaDetail: null,
    evaluation: { grade: "S", primaryTag: "尽力局", evidenceTags: ["伤害最高"], confidence: 0.9 },
    teamAwards: [{ code: "damage", label: "伤害最高", tone: "gold" }],
    sameRoleComparison: null,
    favorite: { note: anonymousFavoriteNote },
    rawJson: null,
  };
}

function createArenaDetail(): MatchDetailView {
  const summary = createArenaSlice("summary", "角斗汇总", "绿洲城 - 花园", 4, 2);
  return {
    ...createNormalDetail(),
    matchId: "fixture-arena",
    gameMode: "LeisureFight",
    modeLabel: "角斗竞技",
    mapGuid: "oasis",
    mapName: "绿洲城 - 花园",
    teamScore: 4,
    opponentScore: 2,
    arenaDetail: {
      summary,
      rounds: [
        createArenaSlice("round-1", "回合 1", "绿洲城 - 花园", 2, 1),
        createArenaSlice("round-2", "回合 2", "绿洲城 - 花园", 2, 1),
      ],
    },
  };
}

function createArenaSlice(key: string, label: string, mapName: string, teamScore: number, opponentScore: number): ArenaDetailSlice {
  return {
    key,
    label,
    result: "win",
    resultLabel: "胜利",
    teamScore,
    opponentScore,
    durationSec: 460,
    beginTs: 1783958400,
    beginLabel: "26/07/14 00:00",
    mapGuid: "oasis",
    mapName,
    teammates: players.slice(0, 6),
    enemies: players.slice(6, 12),
  };
}

function inlineLocalAssets(markup: string) {
  const publicRoot = fileURLToPath(new URL("../public", import.meta.url));
  return markup.replace(/<img\b[^>]*\bsrc="(\/assets\/[^"]+)"[^>]*\/?>/g, (source, assetUrl: string) => {
    const assetPath = resolve(publicRoot, `.${decodeURIComponent(assetUrl)}`);
    if (!assetPath.startsWith(publicRoot) || !existsSync(assetPath)) {
      const className = source.match(/\bclass="([^"]*)"/)?.[1] ?? "";
      if (className.includes("matchListCardHeaderMap")) {
        return `<span class="phase6AssetFallback ${className}" aria-hidden="true"></span>`;
      }
      return source;
    }
    const mime = mimeType(assetPath);
    return source.replace(
      `src="${assetUrl}"`,
      `src="data:${mime};base64,${readFileSync(assetPath).toString("base64")}"`,
    );
  });
}

function mimeType(path: string) {
  if (extname(path).toLowerCase() === ".svg") return "image/svg+xml";
  if (extname(path).toLowerCase() === ".jpg" || extname(path).toLowerCase() === ".jpeg") return "image/jpeg";
  if (extname(path).toLowerCase() === ".webp") return "image/webp";
  return "image/png";
}

function writeFixtures(outputDirectory: string) {
  mkdirSync(outputDirectory, { recursive: true });
  for (const state of ["wide-normal", "wide-arena", "narrow-list", "narrow-detail"] as const) {
    writeFileSync(resolve(outputDirectory, `${state}.html`), renderMatchWorkbenchFixture(state), "utf8");
  }
  process.stdout.write(`匿名对局工作台视觉样例已写入：${outputDirectory}\n`);
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) {
  const outputIndex = process.argv.indexOf("--output");
  const outputDirectory = outputIndex >= 0 ? process.argv[outputIndex + 1] : "";
  if (!outputDirectory) throw new Error("请通过 --output 指定输出目录。");
  writeFixtures(resolve(outputDirectory));
}
