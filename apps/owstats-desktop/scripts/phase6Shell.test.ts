import {
  commandCenterMetrics,
  credentialTone,
  hasDesktopBackend,
  phase6NavItems,
  productErrorMessage,
  readableSyncProgress,
} from "../src/shared/phase6Shell.ts";
import { checkAppUpdate, getPlayerProfile, getUserSettings, logout, saveUserSettings } from "../src/shared/api.ts";
import { formatCompactTimestamp, seasonSyncGateCopy } from "../src/features/command-center/SeasonHealthPanel.tsx";
import type { OnboardingStatus, SyncProgress } from "../src/shared/types.ts";
import { readFileSync } from "node:fs";

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

const navLabels = phase6NavItems.map((item) => item.label).join(",");
assertEqual(navLabels, "对局,战报,生涯 MMR,排行,指挥台", "phase 6 navigation labels");
assertEqual(phase6NavItems.some((item) => item.path === "/sync"), false, "sync page is removed from capsule navigation");
assertEqual(typeof getPlayerProfile, "function", "player profile API wrapper exists");
assertEqual(typeof getUserSettings, "function", "user settings API wrapper exists");
assertEqual(typeof saveUserSettings, "function", "save settings API wrapper exists");
assertEqual(typeof checkAppUpdate, "function", "update check API wrapper exists");
assertEqual(typeof logout, "function", "logout API wrapper exists");

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const seasonHealthSource = readFileSync(new URL("../src/features/command-center/SeasonHealthPanel.tsx", import.meta.url), "utf8");
const commandCenterSource = `${onboardingSource}\n${seasonHealthSource}`;
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");
const styles = [
  readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8"),
  readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8"),
].join("\n");
const commandCenterStyles = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");
const pageShellStyles = readFileSync(new URL("../src/styles/page-shell.css", import.meta.url), "utf8");
assertEqual(/\.commandCenterMetricsRow\s*\{[\s\S]*?border:\s*0;/.test(commandCenterStyles), true, "metrics strip removes its heavy outer border");
assertEqual(/\.commandCenterSyncPanel,[\s\S]*?\.commandCenterAccountPanel[\s\S]*?background:\s*var\(--glass-primary\);/.test(commandCenterStyles), true, "main panels use the primary glass surface");
assertEqual(/@media \(max-width: 1440px\)[\s\S]*?\.commandCenterMetricsRow\s*\{[\s\S]*?grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\);/.test(commandCenterStyles), true, "1440px folds metrics into two columns");
assertEqual(/\.commandCenterAccountPanel \.phase6ProfileAvatar\s*\{[\s\S]*?width:\s*56px;[\s\S]*?height:\s*56px;/.test(commandCenterStyles), true, "account identity uses a compact neutral avatar");
assertEqual(/\.commandCenterTools \.phase6SettingsLauncherGrid\s*\{[\s\S]*?grid-template-columns:\s*repeat\(4, minmax\(0, 1fr\)\);/.test(commandCenterStyles), true, "tools use four columns on wide screens");
assertEqual(/\.commandCenterDangerZone\s*\{[\s\S]*?background:\s*rgba\(248, 231, 229, 0\.34\);/.test(commandCenterStyles), true, "logout stays a light danger treatment");
assertEqual(/\.pageHeader h1\s*\{[\s\S]*?font-weight:\s*700;/.test(pageShellStyles), true, "page titles use the only bold text tier");
assertEqual(/\.commandCenterMetricsRow > span > strong\s*\{[\s\S]*?color:\s*var\(--text-primary\);[\s\S]*?font-weight:\s*400;/.test(commandCenterStyles), true, "metric values stay neutral and regular weight");
assertEqual(/\.commandCenterMetricsRow > span\.(?:good|warn) > strong/.test(commandCenterStyles), false, "metric values do not use green or yellow text emphasis");
assertEqual(/\.commandCenterSectionHeading h2,[\s\S]*?font-weight:\s*400;/.test(commandCenterStyles), true, "command center section titles stay regular weight");
assertEqual(/\.phase6CommandCenter \.phase6Eyebrow\s*\{[\s\S]*?color:\s*var\(--text-secondary\);/.test(commandCenterStyles), true, "command center eyebrow text does not use purple");
assertEqual(/\.phase6CommandCenter \.phase8SeasonStatusStats > span\.(?:good|warn) strong\s*\{[\s\S]*?color:\s*var\(--text-primary\);/.test(commandCenterStyles), true, "season facts do not use green or yellow text");
assertEqual(/\.phase6CommandCenter \.phase6DataHealthGrid span\s*\{[\s\S]*?background:\s*transparent;/.test(commandCenterStyles), true, "health rows remove colored pills");
assertEqual(/\.phase6CommandCenter \.syncTargetStrip,[\s\S]*?background:\s*rgba\(250, 251, 247, 0\.38\);/.test(commandCenterStyles), true, "target strip removes the opaque white inner card");
assertEqual(appSource.includes('<Navigate to="/matches" replace />'), true, "root route redirects to matches");
assertEqual(appSource.includes('to="/onboarding#local-data"'), true, "local data entry targets the command center data section");
assertEqual(appSource.includes('aria-label="查看本地数据"'), true, "local data entry has an accessible name");
assertEqual(appSource.includes("aria-current={false}"), true, "local data entry does not duplicate the current page announcement");
assertEqual(appSource.includes("Bell"), false, "global shell removes the non-functional notification bell");
assertEqual(appSource.includes("preloadDefaultMatchesPage"), false, "global shell no longer preloads matches");
assertEqual(appSource.includes("<RouteFocusManager />"), true, "global shell manages route hash focus");
assertEqual(
  /\.phase6CommandCenter\s*\{\s*max-width:\s*var\(--phase6-content-max\);\s*margin-inline:\s*auto;/.test(styles),
  true,
  "command center is horizontally centered inside the wide workspace",
);
assertEqual(
  /\.commandCenterMainGrid\s*\{[\s\S]*?grid-template-columns:\s*minmax\(0, 2fr\) minmax\(280px, 1fr\);/.test(styles),
  true,
  "command center gives season sync two thirds and account health one third",
);
assertEqual(
  /\.commandCenterMetricsRow\s*\{[\s\S]*?grid-template-columns:\s*repeat\(4, minmax\(0, 1fr\)\);/.test(styles),
  true,
  "command center metrics form one horizontal four-column row",
);
assertEqual(
  /\.phase6RankingWorkspace\s*\{\s*max-width:\s*var\(--phase6-content-max\);\s*margin-inline:\s*auto;/.test(styles),
  true,
  "ranking page is horizontally centered inside the wide workspace",
);
assertEqual(
  /\.phase8SeasonStatusStats\s*\{\s*display: grid;\s*grid-template-columns: repeat\(6, minmax\(0, 1fr\)\);/.test(styles),
  true,
  "command center data facts use one compact six-column row on wide screens",
);
assertEqual(
  /@media \(max-width: 920px\)\s*\{[\s\S]*?\.phase8SeasonStatusStats\s*\{\s*grid-template-columns: repeat\(3, minmax\(0, 1fr\)\);/.test(styles),
  true,
  "command center data facts fall back to three columns on narrow screens",
);
assertEqual(
  /\.syncSeasonWorkbench \.phase8SeasonStatusStats,\s*\.syncSeasonWorkbench \.phase8SeasonStatusHeader,\s*\.syncSeasonWorkbench \.phase8SeasonCoverage,\s*\.syncSeasonWorkbench \.phase8SeasonSyncFooter\s*\{\s*grid-column: 1 \/ -1;/.test(styles),
  true,
  "command center data facts span the workbench width instead of its narrow left column",
);
assertEqual(
  formatCompactTimestamp("2026-07-12T02:56:30Z"),
  "07-12 10:56",
  "compact timestamp removes only the year while preserving local month day and minute",
);
assertEqual(seasonSyncGateCopy(false, false, false, false, false).statusLabel, "需要登录", "missing credentials keep the login status copy");
assertEqual(seasonSyncGateCopy(false, false, false, false, false).syncHint, "登录后可同步", "missing credentials keep the login recovery copy");
assertEqual(seasonSyncGateCopy(true, false, false, false, false).statusLabel, "目标待确认", "ready credentials distinguish a blocked target");
assertEqual(seasonSyncGateCopy(true, false, false, false, false).syncHint, "确认当前目标后可同步", "blocked target explains the required confirmation");
assertEqual(seasonSyncGateCopy(false, false, true, false, false).statusLabel, "账号处理中", "first login identification takes precedence over static login copy");
assertEqual(seasonSyncGateCopy(false, false, true, false, false).syncHint, "账号操作完成后可同步", "first login identification explains the active operation");
assertEqual(seasonSyncGateCopy(true, false, true, false, false).statusLabel, "账号处理中", "target confirmation takes precedence over static target copy");
assertEqual(seasonSyncGateCopy(true, false, true, false, false).syncHint, "账号操作完成后可同步", "target confirmation explains the active operation");
assertEqual(seasonSyncGateCopy(true, true, true, false, true).statusLabel, "账号处理中", "account work has its own status copy");
assertEqual(seasonSyncGateCopy(true, true, true, false, true).syncHint, "账号操作完成后可同步", "account work has its own recovery copy");
assertEqual(seasonSyncGateCopy(true, true, true, true, true).statusLabel, "同步中", "active sync does not fall back to account-processing copy");
assertEqual(seasonSyncGateCopy(true, true, false, false, true).statusLabel, "本地数据可用", "normal ready target preserves local-data status");
assertEqual(seasonSyncGateCopy(true, true, false, false, true).syncHint, "当前目标可同步", "normal ready target remains syncable");
assertEqual(onboardingSource.includes("<PageHeader"), true, "command center uses the shared lightweight page header");
assertEqual(onboardingSource.includes('title="指挥台"'), true, "command center keeps its page title");
assertEqual(onboardingSource.includes("commandCenterMetricsRow"), true, "command center exposes four horizontal status metrics");
assertEqual(onboardingSource.includes("commandCenterMainGrid"), true, "command center uses a two-column work area");
assertEqual(onboardingSource.includes("commandCenterSyncPanel"), true, "season sync is the primary two-thirds panel");
assertEqual(onboardingSource.includes("commandCenterAccountPanel"), true, "account and health use the narrow side panel");
assertEqual(onboardingSource.includes("commandCenterTools"), true, "secondary tools are grouped at the bottom");
assertEqual(onboardingSource.includes("commandCenterDangerZone"), true, "logout is separated as a dangerous operation");
assertEqual(onboardingSource.includes('id="local-data"'), true, "local data shell link has a focusable destination");
assertEqual(onboardingSource.includes("常用英雄"), false, "common heroes move out of the command center first screen");
assertEqual(onboardingSource.includes("成就展示"), false, "achievements move out of the command center first screen");
assertEqual(onboardingSource.includes("getPersonalSeasonReview"), false, "command center no longer requests review-only showcase data");
assertEqual(onboardingSource.includes("phase6PlayerPass"), false, "command center removes the legacy oversized player pass");
assertEqual(onboardingSource.includes("phase6CoverageMeter"), true, "command center shows data coverage");
assertEqual(onboardingSource.includes("phase6DataCoverageCard"), true, "data area keeps one unified coverage card");
assertEqual(onboardingSource.includes("seasonStatuses"), true, "command center reads the selected season status from onboarding status");
assertEqual(onboardingSource.includes("FIXED_COMMAND_CENTER_SEASONS"), false, "command center does not fix the season list");
assertEqual(onboardingSource.includes("seasonInput"), true, "command center accepts an explicit season number");
assertEqual(onboardingSource.includes("BattleTag"), false, "command center never exposes BattleTag input or manual binding copy");
assertEqual(onboardingSource.includes('aria-label="BattleTag"'), false, "command center removes the BattleTag input");
assertEqual(onboardingSource.includes("battleTag"), false, "command center removes BattleTag input state");
assertEqual(onboardingSource.includes("runBindTarget"), false, "command center removes manual target binding");
assertEqual(onboardingSource.includes("切换到此账号"), true, "account mismatch offers an explicit switch action");
assertEqual(onboardingSource.includes("保留原目标"), true, "account mismatch offers an explicit keep action");
assertEqual(onboardingSource.includes("重新绑定"), true, "retryable binding failure offers a retry action");
assertEqual(onboardingSource.includes("重新识别"), true, "unidentified account offers a fresh login identification action");
assertEqual(onboardingSource.includes("credentialReady={credentialReady}"), true, "season card receives the real credential state");
assertEqual(onboardingSource.includes("targetSyncAllowed={Boolean(targetStatus?.syncAllowed)}"), true, "season card receives the backend target gate separately");
assertEqual(onboardingSource.includes("accountOperationBusy={accountOperationBusy}"), true, "season card receives the account operation state separately");
assertEqual(onboardingSource.includes("credentialReady={syncTargetReady}"), false, "season card never receives a mixed gate disguised as credential state");
assertEqual(
  /disabled=\{!credentialReady \|\| !targetSyncAllowed \|\| accountOperationBusy \|\| isSyncBlocked \|\| isSyncing\}/.test(seasonHealthSource),
  true,
  "season sync button combines credential, target, account-operation and sync gates",
);
assertEqual(onboardingSource.includes("const accountOperationBusy = loggingIn || loggingOut || targetAction !== null"), true, "account actions share one busy gate");
assertEqual(seasonHealthSource.includes("credentialReady && targetSyncAllowed && !accountOperationBusy"), true, "account actions suspend season sync without changing credential semantics");
assertEqual(onboardingSource.includes("accountOperationLockRef"), true, "account handlers have a synchronous re-entry guard");
assertEqual(onboardingSource.includes("dashboardRequestGenerationRef"), true, "dashboard refreshes invalidate stale responses");
assertEqual(onboardingSource.includes("mountedRef"), true, "dashboard responses cannot update an unmounted command center");
assertEqual(onboardingSource.includes("primaryTargetActionRef"), true, "target recovery exposes a focus destination");
assertEqual(onboardingSource.includes("previousPrimaryTargetStateRef"), true, "target recovery focus is not stolen on same-state rerenders");
assertEqual(
  /catch \(err\) \{[\s\S]*?const loginError = `登录或识别失败[\s\S]*?await loadDashboard\(\);[\s\S]*?setError\(loginError\)/.test(onboardingSource),
  true,
  "login refresh completes before the final recoverable error is shown",
);
assertEqual(/async function runClearTarget\(\)[\s\S]*?setNotice\(result\.target\.message\)/.test(onboardingSource), true, "clear action uses the backend recovery message");
assertEqual(onboardingSource.includes("{!hasTargetPrimaryAction && ("), true, "target recovery does not duplicate the bottom login CTA");
assertEqual(onboardingSource.includes("let disposed = false"), true, "late event subscriptions are disposed safely");
assertEqual(onboardingSource.includes("if (disposed)"), true, "late event listeners immediately unlisten after cleanup");
assertEqual(onboardingSource.includes("function acquireOperationLock()"), true, "account and season actions share one synchronous lock");
assertEqual(
  /async function runSeasonSync\(season: number\)[\s\S]*?if \(!acquireOperationLock\(\)\)[\s\S]*?finally[\s\S]*?releaseOperationLock\(\)/.test(onboardingSource),
  true,
  "season sync owns and releases the shared operation lock",
);
assertEqual(onboardingSource.includes("disabled={accountOperationBusy || syncingSeason !== null || !targetStatus.canConfirmSwitch}"), true, "switch confirmation is disabled during season sync");
assertEqual(onboardingSource.includes("disabled={!hasCurrentTarget || accountOperationBusy || syncingSeason !== null}"), true, "clear is disabled during season sync");
assertEqual(onboardingSource.includes("disabled={!credentialReady || accountOperationBusy || syncingSeason !== null}"), true, "logout is disabled during season sync");
assertEqual((onboardingSource.match(/accountOperationBusy \|\| syncingSeason !== null/g) ?? []).length >= 6, true, "all account action buttons are disabled during season sync");
assertEqual(onboardingSource.includes("function invalidateDashboardCache()"), true, "account changes invalidate the cached dashboard snapshot");
assertEqual(
  /function beginAccountOperation\(\)[\s\S]*?invalidateDashboardCache\(\)/.test(onboardingSource),
  true,
  "every account operation invalidates stale cached identity before changing state",
);
assertEqual(
  /useEffect\(\(\) => \{[\s\S]*?accountOperationBusy \|\| syncingSeason !== null[\s\S]*?button\.disabled[\s\S]*?button\.focus\(\)[\s\S]*?document\.activeElement === button[\s\S]*?previousPrimaryTargetStateRef\.current = primaryTargetState[\s\S]*?\}, \[primaryTargetState, accountOperationBusy, syncingSeason\]\)/.test(onboardingSource),
  true,
  "target recovery focus is recorded only after the enabled button receives focus",
);
assertEqual(onboardingSource.includes("phase8SeasonStatusGrid"), true, "command center renders the selected season card");
assertEqual(commandCenterSource.includes("phase8SeasonStatusCard"), true, "command center renders individual season status cards");
assertEqual(onboardingSource.includes("const COMMAND_CENTER_SEASONS = [21, 22, 23] as const"), true, "command center exposes fixed season segments");
assertEqual(onboardingSource.includes('aria-label="选择同步赛季"'), true, "season picker has a Chinese accessible name");
assertEqual(onboardingSource.includes('aria-label="赛季号"'), false, "numeric season input is removed");
assertEqual(commandCenterSource.includes("`同步 S${seasonStatus.season}`"), true, "season sync button text is generated per season");
assertEqual(onboardingSource.includes("同步全部"), false, "command center has no sync-all action");
assertEqual(onboardingSource.includes("currentSeason"), false, "command center does not introduce global current season state");
assertEqual(onboardingSource.includes("dump_season_json"), false, "command center does not expose diagnostic dump command");
assertEqual(onboardingSource.includes("dumpSeason"), false, "command center does not expose diagnostic dump API wrapper");
assertEqual(onboardingSource.includes("commandCenterMetricsRow"), true, "top metrics own credential, match count, coverage and last sync facts");
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
assertEqual(onboardingSource.indexOf("onClick={runUpdateCheck}") < onboardingSource.indexOf("onClick={runLogin}"), true, "header update action appears before account actions");
assertEqual(onboardingSource.indexOf("onClick={runLogin}") < onboardingSource.indexOf("onClick={runLogout}"), true, "dangerous logout remains after account actions");
assertEqual(onboardingSource.includes("phase6NextActions"), false, "command center does not duplicate capsule navigation with next-step entries");
assertEqual(onboardingSource.includes("CommandEntry"), false, "command center removes duplicated next-step entry component");
assertEqual(rankingSource.includes("<PageHeader"), true, "ranking page uses the shared lightweight page header");
assertEqual(rankingSource.includes('<header className="phase6PageHeader phase6RankingHeader">'), false, "ranking page does not restore the old thick header card");
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
assertEqual(
  productErrorMessage(new Error("合成视觉验收错误：排行请求失败，请重新加载。")),
  "合成视觉验收错误：排行请求失败，请重新加载。",
  "JavaScript Error prefixes must not leak into Chinese product copy",
);
assertEqual(
  productErrorMessage(new Error("network request failed")),
  "请求失败，请稍后重新加载。",
  "non-Chinese implementation errors must fall back to stable Chinese product copy",
);
assertEqual(hasDesktopBackend({}), false, "plain browser has no desktop backend");
assertEqual(hasDesktopBackend({ __TAURI_INTERNALS__: {} }), true, "tauri internals indicate desktop backend");
