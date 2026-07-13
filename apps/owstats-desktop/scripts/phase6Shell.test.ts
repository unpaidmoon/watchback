import {
  commandCenterMetrics,
  credentialTone,
  hasDesktopBackend,
  phase6NavItems,
  productErrorMessage,
  readableSyncProgress,
} from "../src/shared/phase6Shell.ts";
import { checkAppUpdate, getPlayerProfile, getUserSettings, logout, saveUserSettings } from "../src/shared/api.ts";
import type { OnboardingStatus, SyncProgress } from "../src/shared/types.ts";
import { readFileSync } from "node:fs";

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

const navLabels = phase6NavItems.map((item) => item.label).join(",");
assertEqual(navLabels, "指挥台,对局,战报,生涯 MMR,排行", "phase 6 navigation labels");
assertEqual(phase6NavItems.some((item) => item.path === "/sync"), false, "sync page is removed from capsule navigation");
assertEqual(typeof getPlayerProfile, "function", "player profile API wrapper exists");
assertEqual(typeof getUserSettings, "function", "user settings API wrapper exists");
assertEqual(typeof saveUserSettings, "function", "save settings API wrapper exists");
assertEqual(typeof checkAppUpdate, "function", "update check API wrapper exists");
assertEqual(typeof logout, "function", "logout API wrapper exists");

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const seasonHealthSource = readFileSync(new URL("../src/features/command-center/SeasonHealthPanel.tsx", import.meta.url), "utf8");
const commandCenterSource = `${onboardingSource}\n${seasonHealthSource}`;
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");
const styles = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");
assertEqual(onboardingSource.includes("phase6PlayerPass"), true, "command center uses a player pass identity strip");
assertEqual(onboardingSource.includes("phase6PlayerPassRail"), true, "player pass uses a compact visual rail instead of a large diagonal block");
assertEqual(onboardingSource.includes("phase6ProfileInlineStats"), true, "player pass places endorsement and play time under the title");
assertEqual(onboardingSource.includes("phase6ProfileShowcase"), true, "profile card adds a right-side hero and achievement showcase");
assertEqual(onboardingSource.includes("phase6ProfileShowcaseGroup"), true, "profile showcase is split into titled hero and achievement groups");
assertEqual(onboardingSource.includes("常用英雄"), true, "profile showcase labels the common heroes group");
assertEqual(onboardingSource.includes("成就展示"), true, "profile showcase labels the achievements group");
assertEqual(onboardingSource.includes("phase6ProfileHeroStrip"), true, "profile showcase uses a horizontal three-hero strip");
assertEqual(onboardingSource.includes("mainHeroes.slice(0, 3)"), true, "profile showcase limits common heroes to three");
assertEqual(onboardingSource.includes("Array.from({ length: 3 })"), true, "profile showcase renders three stable hero and achievement slots");
assertEqual(onboardingSource.includes("phase6ProfileHeroName"), true, "profile showcase renders hero names under avatars");
assertEqual(onboardingSource.includes("phase6ProfileAchievementBadges"), true, "profile showcase shows top active achievements");
assertEqual(onboardingSource.includes("phase6ProfileAchievementBadgeFrame"), false, "achievement badges no longer use light circular backing frames");
assertEqual(onboardingSource.includes("phase6ProfileAchievementToken"), true, "achievement badges keep only a dark token treatment");
assertEqual(onboardingSource.includes("phase6ProfileAchievementName"), true, "profile showcase renders achievement names under badges");
assertEqual(onboardingSource.includes("phase6ProfileAchievementInset"), true, "achievement group keeps a dedicated achievement treatment");
assertEqual(styles.includes("grid-template-rows: 56px 30px"), true, "profile hero and achievement showcase items share fixed media and label rows");
assertEqual(/\.phase6ProfileAchievementToken\s*\{\s*width: 56px;\s*height: 56px;/.test(styles), true, "profile achievement tokens match hero avatar size");
assertEqual(styles.includes("rgb(255 252 242 / 0.82)"), true, "achievement showcase uses a light token-tinted background");
assertEqual(styles.includes("linear-gradient(145deg, rgb(23 32 51 / 0.98), rgb(36 47 68 / 0.95))"), false, "achievement showcase no longer uses the overly dark embedded panel");
assertEqual(onboardingSource.includes("activeTopAchievements"), true, "profile showcase uses shared active achievement ranking");
assertEqual(onboardingSource.includes("getPersonalSeasonReview"), true, "profile showcase loads review data for heroes and achievements");
assertEqual(onboardingSource.includes("phase6PassStatusTrack"), false, "player pass removes duplicated account status chips");
assertEqual(onboardingSource.includes("phase6UnifiedCommandPanel"), true, "command center is consolidated into one player pass control panel");
assertEqual(onboardingSource.includes("phase6CoverageMeter"), true, "command center shows data coverage");
assertEqual(onboardingSource.includes("phase6DataCoverageCard"), true, "data area keeps one unified coverage card");
assertEqual(onboardingSource.includes("seasonStatuses"), true, "command center reads fixed season status cards from onboarding status");
assertEqual(onboardingSource.includes("FIXED_COMMAND_CENTER_SEASONS"), true, "command center has an explicit fixed season list");
assertEqual(onboardingSource.includes("phase8SeasonStatusGrid"), true, "command center renders a fixed season card grid");
assertEqual(commandCenterSource.includes("phase8SeasonStatusCard"), true, "command center renders individual season status cards");
assertEqual(onboardingSource.includes("S21"), true, "command center includes S21 card copy");
assertEqual(onboardingSource.includes("S22"), true, "command center includes S22 card copy");
assertEqual(onboardingSource.includes("S23"), true, "command center includes S23 card copy");
assertEqual(commandCenterSource.includes("`同步 S${seasonStatus.season}`"), true, "season sync button text is generated per season");
assertEqual(onboardingSource.includes("同步全部"), false, "command center has no sync-all action");
assertEqual(onboardingSource.includes("currentSeason"), false, "command center does not introduce global current season state");
assertEqual(onboardingSource.includes("dump_season_json"), false, "command center does not expose diagnostic dump command");
assertEqual(onboardingSource.includes("dumpSeason"), false, "command center does not expose diagnostic dump API wrapper");
assertEqual(onboardingSource.includes("phase6DataFacts"), true, "data coverage card owns credential, match count, and last sync facts");
assertEqual(onboardingSource.includes("phase6DataHealthGrid"), true, "data coverage card owns health checks");
assertEqual(onboardingSource.includes("phase6SyncTimeline"), false, "command center removes separate sync status card");
assertEqual(onboardingSource.includes("phase6HealthChecklist"), false, "command center removes separate health checklist card");
assertEqual(onboardingSource.includes("数据覆盖度"), true, "command center labels data coverage in Chinese");
assertEqual(onboardingSource.includes("同步状态"), false, "command center no longer labels a separate sync timeline");
assertEqual(onboardingSource.includes("数据健康检查"), false, "command center no longer labels a separate health card");
assertEqual(onboardingSource.includes("版本号"), true, "settings exposes version entry");
assertEqual(onboardingSource.includes("日志文件"), true, "settings exposes log file entry");
assertEqual(onboardingSource.includes("数据存放路径"), true, "settings exposes data path entry");
assertEqual(onboardingSource.includes("Github地址"), true, "settings exposes GitHub entry");
assertEqual(onboardingSource.includes("phase6SettingsLauncherGrid"), true, "settings uses a compact launcher grid");
assertEqual(onboardingSource.includes("phase6AccountBanner"), false, "old diagonal account banner is removed");
assertEqual(onboardingSource.includes("phase6CommandMainGrid"), false, "separate data/settings card grid is removed");
assertEqual(onboardingSource.includes("phase6DataOpsPanel"), false, "separate season data card is removed");
assertEqual(onboardingSource.includes("phase6SettingsPanel"), false, "separate settings card is removed");
assertEqual(onboardingSource.includes("phase6CommandLayout"), false, "command center no longer uses the three-card layout");
assertEqual(onboardingSource.includes('label="角色ID"'), false, "account banner does not repeat role id in facts");
assertEqual(onboardingSource.includes("dashboardCache"), true, "command center keeps dashboard data in module cache");
assertEqual(onboardingSource.includes("applyDashboardSnapshot"), true, "command center can render cached dashboard data before refresh");
assertEqual(onboardingSource.includes("phase6ThemeSelector"), false, "command center settings removes theme selector controls");
assertEqual(onboardingSource.includes("phase6CompactSettingsActions"), false, "settings account actions move into the launcher grid");
assertEqual(onboardingSource.includes("StatusBadge tone={credentialTone(status?.credentialStatus.state)}"), false, "profile card removes the right-side login status capsule");
assertEqual(onboardingSource.indexOf("onClick={runLogin}") < onboardingSource.indexOf("onClick={runUpdateCheck}"), true, "login action appears before update action");
assertEqual(onboardingSource.indexOf("onClick={runUpdateCheck}") < onboardingSource.indexOf("onClick={runLogout}"), true, "update action appears before logout action");
assertEqual(onboardingSource.includes("phase6NextActions"), false, "command center does not duplicate capsule navigation with next-step entries");
assertEqual(onboardingSource.includes("CommandEntry"), false, "command center removes duplicated next-step entry component");
assertEqual(rankingSource.includes('<header className="phase6PageHeader phase6RankingHeader">'), false, "ranking page removes header card");
assertEqual(rankingSource.includes('const heroSortKeys: RankingSortKey[] = ["matches", "winRate", "playTimeHours", "kda"]'), true, "hero ranking sortable keys exclude rating");
assertEqual(rankingSource.includes('const mapSortKeys: RankingSortKey[] = ["matches", "winRate", "playTimeHours", "kda"]'), true, "map ranking sortable keys exclude best hero and rating");
assertEqual(onboardingSource.includes("队友分析"), false, "command center does not duplicate friend analysis navigation");

assertEqual(credentialTone("ready"), "good", "current-account credential is good");
assertEqual(credentialTone("missing"), "warn", "missing credential needs attention");
assertEqual(credentialTone("currentPlayerMissing"), "warn", "unidentified current account needs attention");
assertEqual(credentialTone("failed"), "bad", "failed credential is bad");

const status: OnboardingStatus = {
  selectedSeason: 22,
  availableSeasons: [22],
  canEnterReview: true,
  credentialStatus: {
    state: "ready",
    message: "当前登录账号可用",
    exposesSecret: false,
    roleId: 123,
    savedAt: "2026-06-28T05:40:00Z",
  },
  localData: {
    dataSource: "sqlite",
    season: 22,
    presetMatches: 186,
    detailMatches: 142,
    lastSyncedOrImportedAt: "2026-06-28 05:40",
    freshnessRisk: "fresh",
  },
  seasonStatuses: [
    {
      season: 21,
      matchCount: 0,
      detailCount: 0,
      normalMatchCount: 0,
      arenaMatchCount: 0,
      failedDetailCount: 0,
      lastSyncedOrImportedAt: null,
      coveragePercent: 0,
    },
    {
      season: 22,
      matchCount: 186,
      detailCount: 142,
      normalMatchCount: 140,
      arenaMatchCount: 46,
      failedDetailCount: 1,
      lastSyncedOrImportedAt: "2026-06-28 05:40",
      coveragePercent: 76,
    },
    {
      season: 23,
      matchCount: 0,
      detailCount: 0,
      normalMatchCount: 0,
      arenaMatchCount: 0,
      failedDetailCount: 0,
      lastSyncedOrImportedAt: null,
      coveragePercent: 0,
    },
  ],
  health: { readable: true, warnings: [] },
};

const metrics = commandCenterMetrics(status);
assertEqual(typeof metrics[0].label, "string", "first metric label is readable");
assertEqual(typeof metrics[0].value, "string", "credential metric value is readable");
assertEqual(metrics[1].value, "S22", "season metric value");
assertEqual(metrics[2].value, "186", "match count value");
assertEqual(metrics[3].value, "142", "detail count value");

const progress: SyncProgress = {
  season: 22,
  phase: "details",
  processed: 8,
  total: 20,
  currentMatchId: null,
  failedDetails: 1,
  latestErrors: [],
};

assertEqual(readableSyncProgress(progress).includes("8/20"), true, "sync progress includes count");
assertEqual(
  productErrorMessage("TypeError: Cannot read properties of undefined (reading 'invoke')").includes("Tauri"),
  true,
  "tauri browser error becomes product copy",
);
assertEqual(hasDesktopBackend({}), false, "plain browser has no desktop backend");
assertEqual(hasDesktopBackend({ __TAURI_INTERNALS__: {} }), true, "tauri internals indicate desktop backend");
