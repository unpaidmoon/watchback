import {
  commandCenterMetrics,
  credentialTone,
  hasDesktopBackend,
  phase6NavItems,
  productErrorMessage,
  readableSyncProgress,
} from "../src/shared/phase6Shell.ts";
import { checkAppUpdate, getPlayerProfile, getUserSettings, listAccounts, saveUserSettings } from "../src/shared/api.ts";
import { formatCompactTimestamp, seasonSyncGateCopy } from "../src/features/command-center/SeasonHealthPanel.tsx";
import * as seasonSyncWorkbench from "../src/features/command-center/SeasonSyncWorkbench.tsx";
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
assertEqual(typeof listAccounts, "function", "account manager API wrapper exists");

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const seasonHealthSource = readFileSync(new URL("../src/features/command-center/SeasonHealthPanel.tsx", import.meta.url), "utf8");
const seasonWorkbenchSource = readFileSync(new URL("../src/features/command-center/SeasonSyncWorkbench.tsx", import.meta.url), "utf8");
const accountIsolationSource = readFileSync(new URL("../src/features/command-center/accountOperationIsolation.ts", import.meta.url), "utf8");
const commandCenterSource = `${onboardingSource}\n${seasonHealthSource}\n${seasonWorkbenchSource}`;
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");
const styles = [
  readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8"),
  readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8"),
].join("\n");
const commandCenterStyles = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");
const pageShellStyles = readFileSync(new URL("../src/styles/page-shell.css", import.meta.url), "utf8");
const { resolveCommandCenterSeason } = seasonSyncWorkbench;
const runSeasonQuickUpdate = Reflect.get(seasonSyncWorkbench, "runSeasonQuickUpdate") as undefined | ((season: number, onQuickUpdate: (season: number) => void) => void);
assertEqual(typeof runSeasonQuickUpdate, "function", "row updates expose a directly testable quick-update handler");
if (runSeasonQuickUpdate) {
  let selectedSeason = 23;
  let updatedSeason: number | null = null;
  runSeasonQuickUpdate(22, (season) => {
    updatedSeason = season;
  });
  assertEqual(updatedSeason, 22, "row updates invoke quick update with the row season");
  assertEqual(selectedSeason, 23, "row updates do not mutate the selected season");
}
assertEqual(/\.seasonSyncWorkbench\s*\{[\s\S]*?grid-template-columns:\s*minmax\(220px, 1fr\) minmax\(0, 2fr\)/.test(commandCenterStyles), true, "sync controls and ledger use the confirmed 1:2 layout");
assertEqual(/\.syncedSeasonLedgerRow\s*\{[\s\S]*?min-height:\s*44px/.test(commandCenterStyles), true, "ledger rows meet the interaction target");
assertEqual(/\.syncedSeasonUpdateButton\s*\{[\s\S]*?min-height:\s*44px/.test(commandCenterStyles), true, "row update controls meet the interaction target");
assertEqual(/grid-template-columns:\s*minmax\(64px, 0\.55fr\) minmax\(150px, 1\.5fr\) minmax\(88px, 0\.75fr\) minmax\(112px, 0\.9fr\) 80px;/.test(commandCenterStyles), true, "ledger reserves a stable operation column for the updating label");
assertEqual(/\.syncedSeasonUpdateButton\s*\{[\s\S]*?width:\s*80px;[\s\S]*?min-width:\s*80px;/.test(commandCenterStyles), true, "row update controls keep the same width while running");
assertEqual(/\.syncedSeasonCoverageTrack > span\s*\{[\s\S]*?background:\s*var\(--data-primary\)/.test(commandCenterStyles), true, "coverage uses the approved data color");
assertEqual(/@media \(max-width: 920px\)[\s\S]*?\.seasonSyncWorkbench\s*\{[\s\S]*?grid-template-columns:\s*1fr/.test(commandCenterStyles), true, "narrow layout stacks control above ledger");
assertEqual(/\.commandCenterMetricsRow\s*\{[\s\S]*?border:\s*0;/.test(commandCenterStyles), true, "metrics strip removes its heavy outer border");
assertEqual(/\.commandCenterSyncPanel,[\s\S]*?\.commandCenterAccountPanel[\s\S]*?background:\s*var\(--glass-primary\);/.test(commandCenterStyles), true, "main panels use the primary glass surface");
assertEqual(/@media \(max-width: 1440px\)[\s\S]*?\.commandCenterMetricsRow\s*\{[\s\S]*?grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\);/.test(commandCenterStyles), true, "1440px folds metrics into two columns");
assertEqual(/\.commandCenterAccountPanel \.phase6ProfileAvatar\s*\{[\s\S]*?width:\s*56px;[\s\S]*?height:\s*56px;/.test(commandCenterStyles), true, "account identity uses a compact neutral avatar");
assertEqual(/\.commandCenterTools \.phase6SettingsLauncherGrid\s*\{[\s\S]*?grid-template-columns:\s*repeat\(4, minmax\(0, 1fr\)\);/.test(commandCenterStyles), true, "tools use four columns on wide screens");
assertEqual(/\.phase6CommandCenter \.accountManagerPanel\s*\{[\s\S]*?background:\s*var\(--glass-secondary\);/.test(commandCenterStyles), true, "account manager uses the approved secondary glass surface");
assertEqual(/\.pageHeader h1\s*\{[\s\S]*?font-weight:\s*700;/.test(pageShellStyles), true, "page titles use the only bold text tier");
assertEqual(/\.commandCenterMetricsRow > span > strong\s*\{[\s\S]*?color:\s*var\(--text-primary\);[\s\S]*?font-weight:\s*400;/.test(commandCenterStyles), true, "metric values stay neutral and regular weight");
assertEqual(/\.commandCenterMetricsRow > span\.(?:good|warn) > strong/.test(commandCenterStyles), false, "metric values do not use green or yellow text emphasis");
assertEqual(/\.commandCenterSectionHeading h2,[\s\S]*?font-weight:\s*400;/.test(commandCenterStyles), true, "command center section titles stay regular weight");
assertEqual(/\.phase6CommandCenter \.phase6Eyebrow\s*\{[\s\S]*?color:\s*var\(--text-secondary\);/.test(commandCenterStyles), true, "command center eyebrow text does not use purple");
assertEqual(/\.syncedSeasonLedgerRow\s*\{[\s\S]*?color:\s*var\(--text-primary\);/.test(commandCenterStyles), true, "season ledger facts keep neutral text");
assertEqual(/\.phase6CommandCenter \.phase6DataHealthGrid span\s*\{[\s\S]*?background:\s*transparent;/.test(commandCenterStyles), true, "health rows remove colored pills");
assertEqual(/\.phase6CommandCenter \.syncTargetStrip\s*\{[\s\S]*?background:\s*rgba\(250, 251, 247, 0\.38\);/.test(commandCenterStyles), true, "target strip removes the opaque white inner card");
assertEqual(/\.phase6CommandCenter \.phase6DataCoverageCard\s*\{[\s\S]*?border:\s*0;[\s\S]*?border-radius:\s*0;[\s\S]*?background:\s*transparent;[\s\S]*?box-shadow:\s*none;/.test(commandCenterStyles), true, "coverage card is flattened into the account surface");
assertEqual(/\.syncedSeasonLedgerRow\s*\{[\s\S]*?border-bottom:\s*1px solid var\(--glass-border\);/.test(commandCenterStyles), true, "season ledger rows use dividers instead of floating cards");
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
  formatCompactTimestamp("2026-07-12T02:56:30Z"),
  "07-12 10:56",
  "compact timestamp removes only the year while preserving local month day and minute",
);
assertEqual(resolveCommandCenterSeason(21, [23, 22, 21, 20, 19, 18, 17, 16, 15], 23), 21, "valid user selection survives refresh");
assertEqual(resolveCommandCenterSeason(24, [23, 22, 21, 20, 19, 18, 17, 16, 15], 23), 23, "invalid selection falls back to current season");
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
assertEqual(onboardingSource.includes("<AccountManager"), true, "account manager replaces the legacy logout danger zone");
assertEqual(onboardingSource.includes("commandCenterDangerZone"), false, "legacy logout danger zone is removed");
assertEqual(onboardingSource.includes('id="local-data"'), true, "local data shell link has a focusable destination");
assertEqual(onboardingSource.includes("常用英雄"), false, "common heroes move out of the command center first screen");
assertEqual(onboardingSource.includes("成就展示"), false, "achievements move out of the command center first screen");
assertEqual(onboardingSource.includes("getPersonalSeasonReview"), false, "command center no longer requests review-only showcase data");
assertEqual(onboardingSource.includes("phase6PlayerPass"), false, "command center removes the legacy oversized player pass");
assertEqual(onboardingSource.includes("phase6CoverageMeter"), true, "command center shows data coverage");
assertEqual(onboardingSource.includes("phase6DataCoverageCard"), true, "data area keeps one unified coverage card");
assertEqual(onboardingSource.includes("seasonStatuses"), true, "command center reads the selected season status from onboarding status");
assertEqual(onboardingSource.includes("COMMAND_CENTER_SEASONS"), false, "command center does not fix the season list");
assertEqual(onboardingSource.includes("selectedSeason"), true, "command center keeps page-local season selection");
assertEqual(onboardingSource.includes("BattleTag"), false, "command center never exposes BattleTag input or manual binding copy");
assertEqual(onboardingSource.includes('aria-label="BattleTag"'), false, "command center removes the BattleTag input");
assertEqual(onboardingSource.includes("activeAccount?.battleTag"), true, "command center uses the active account BattleTag as its safe identity surface");
assertEqual(onboardingSource.includes("runBindTarget"), false, "command center removes manual target binding");
assertEqual(onboardingSource.includes("切换到此账号"), true, "account mismatch offers an explicit switch action");
assertEqual(onboardingSource.includes("保留原目标"), true, "account mismatch offers an explicit keep action");
assertEqual(onboardingSource.includes("重新绑定"), true, "retryable binding failure offers a retry action");
assertEqual(onboardingSource.includes("重新识别"), true, "unidentified account offers a fresh login identification action");
assertEqual(onboardingSource.includes("credentialReady={credentialReady}"), true, "season workbench receives the real credential state");
assertEqual(onboardingSource.includes("targetSyncAllowed={Boolean(targetStatus?.syncAllowed)}"), true, "season workbench receives the backend target gate separately");
assertEqual(onboardingSource.includes("accountOperationBusy={accountOperationBusy}"), true, "season workbench receives the account operation state separately");
assertEqual(onboardingSource.includes("credentialReady={syncTargetReady}"), false, "season workbench never receives a mixed gate disguised as credential state");
assertEqual(
  /disabled=\{!credentialReady \|\| !targetSyncAllowed \|\| accountOperationBusy \|\| syncingSeason !== null \|\| selectedBackgroundBusy\}/.test(seasonWorkbenchSource),
  true,
  "season sync button combines credential, target, account-operation, foreground and background sync gates",
);
assertEqual(onboardingSource.includes("const accountOperationBusy = busyAccountAction !== null || targetAction !== null"), true, "account actions share one busy gate");
assertEqual(seasonWorkbenchSource.includes("seasonSyncGateCopy("), true, "account actions use the shared season gate copy without changing credential semantics");
assertEqual(onboardingSource.includes("accountOperationLockRef"), true, "account handlers have a synchronous re-entry guard");
assertEqual(onboardingSource.includes("dashboardRequestGenerationRef"), true, "dashboard refreshes invalidate stale responses");
assertEqual(onboardingSource.includes("mountedRef"), true, "dashboard responses cannot update an unmounted command center");
assertEqual(onboardingSource.includes("primaryTargetActionRef"), true, "target recovery exposes a focus destination");
assertEqual(onboardingSource.includes("previousPrimaryTargetStateRef"), true, "target recovery focus is not stolen on same-state rerenders");
assertEqual(
  /runAddAccountOperation[\s\S]*?await dependencies\.refreshAccounts\(\);[\s\S]*?dependencies\.setNotice\(result\.message\)/.test(accountIsolationSource),
  true,
  "account login refresh completes before the final recoverable error is shown",
);
assertEqual(/async function runClearTarget\(\)[\s\S]*?setNotice\(result\.target\.message\)/.test(onboardingSource), true, "clear action uses the backend recovery message");
assertEqual(onboardingSource.includes("hasTargetPrimaryAction"), false, "account manager removes the duplicate bottom login CTA gate");
assertEqual(onboardingSource.includes("let disposed = false"), true, "late event subscriptions are disposed safely");
assertEqual(onboardingSource.includes("if (disposed)"), true, "late event listeners immediately unlisten after cleanup");
assertEqual(onboardingSource.includes("function acquireOperationLock()"), true, "account and season actions share one synchronous lock");
assertEqual(
  /async function runSeasonSync\(season: number, mode: SyncMode\)[\s\S]*?if \(!acquireOperationLock\(\)\)[\s\S]*?finally[\s\S]*?releaseOperationLock\(\)/.test(onboardingSource),
  true,
  "season sync owns and releases the shared operation lock",
);
assertEqual(
  /async function runSeasonSync\(season: number, mode: SyncMode\)[\s\S]*?if \(!acquireOperationLock\(\)\)\s*\{[\s\S]*?setSyncError\(/.test(onboardingSource),
  true,
  "season sync lock failures stay beside the sync control",
);
assertEqual(
  /function selectSyncSeason\(season: number\)[\s\S]*?setSyncError\(null\)[\s\S]*?setSyncNotice\(null\)[\s\S]*?setSelectedSeason\(season\)/.test(onboardingSource),
  true,
  "changing seasons clears stale sync feedback",
);
assertEqual(onboardingSource.includes("disabled={accountOperationBusy || syncingSeason !== null || !targetStatus.canConfirmSwitch}"), true, "switch confirmation is disabled during season sync");
assertEqual(onboardingSource.includes("disabled={!hasCurrentTarget || accountOperationBusy || syncingSeason !== null}"), true, "clear is disabled during season sync");
assertEqual((onboardingSource.match(/accountOperationBusy \|\| syncingSeason !== null/g) ?? []).length >= 5, true, "all account and target actions are disabled during season sync");
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
assertEqual(onboardingSource.includes("<SeasonSyncWorkbench"), true, "command center renders the dynamic season workbench");
assertEqual(seasonWorkbenchSource.includes('aria-label="选择同步赛季"'), true, "dynamic season select has a Chinese accessible name");
assertEqual(seasonWorkbenchSource.includes("syncableSeasons.map"), true, "season options come from the backend catalog");
assertEqual(seasonWorkbenchSource.includes("已同步数据"), true, "ledger has the confirmed Chinese heading");
assertEqual(seasonWorkbenchSource.includes("详情覆盖度"), true, "ledger shows coverage");
assertEqual(seasonWorkbenchSource.includes("本地对局"), true, "ledger shows local matches");
assertEqual(seasonWorkbenchSource.includes("最近同步"), true, "ledger shows compact time");
assertEqual(seasonWorkbenchSource.includes("普通对局"), false, "ledger omits non-approved columns");
assertEqual(seasonWorkbenchSource.includes("角斗对局"), false, "ledger omits non-approved columns");
assertEqual(seasonWorkbenchSource.includes("aria-pressed"), true, "ledger rows expose their selected state");
assertEqual(onboardingSource.includes('aria-label="赛季号"'), false, "numeric season input is removed");
assertEqual(seasonWorkbenchSource.includes('onFullFetch: (season: number) => void'), true, "workbench exposes an explicit full-fetch action");
assertEqual(seasonWorkbenchSource.includes('onQuickUpdate: (season: number) => void'), true, "workbench exposes an explicit row update action");
assertEqual(seasonWorkbenchSource.includes('"全量抓取"'), true, "selected-season primary action is named full fetch");
assertEqual(seasonWorkbenchSource.includes('"更新"'), true, "synced-season rows expose a compact update action");
assertEqual(/runSeasonQuickUpdate\(seasonStatus\.season, onQuickUpdate\)/.test(seasonWorkbenchSource), true, "row update uses its own season through the tested handler");
assertEqual(/<button[^>]*className=.*syncedSeasonLedgerRow[\s\S]*?<button/.test(seasonWorkbenchSource), false, "season rows do not nest buttons");
assertEqual(onboardingSource.includes("同步全部"), false, "command center has no sync-all action");
assertEqual(onboardingSource.includes("status.currentSeason"), true, "command center consumes the backend current season without global state");
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
assertEqual(onboardingSource.includes("DASHBOARD_SEASON"), false, "command center cache has no fixed-season key");
assertEqual(onboardingSource.includes("applyDashboardSnapshot"), true, "command center can render cached dashboard data before refresh");
assertEqual(onboardingSource.includes("phase6ThemeSelector"), false, "command center settings removes theme selector controls");
assertEqual(onboardingSource.includes("phase6CompactSettingsActions"), false, "settings account actions move into the launcher grid");
assertEqual(onboardingSource.includes("StatusBadge tone={credentialTone(status?.credentialStatus.state)}"), false, "profile card removes the right-side login status capsule");
assertEqual(onboardingSource.indexOf("onClick={runUpdateCheck}") < onboardingSource.lastIndexOf("<AccountManager"), true, "header update action appears before account management");
assertEqual(onboardingSource.indexOf("commandCenterTools") < onboardingSource.lastIndexOf("<AccountManager"), true, "account management appears after the existing command-center content");
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
  currentSeason: 23,
  syncableSeasons: [23, 22, 21, 20, 19, 18, 17, 16, 15],
  availableSeasons: [22],
  canEnterReview: true,
  credentialStatus: {
    state: "ready",
    message: "当前登录账号可用",
    exposesSecret: false,
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

assertEqual(readableSyncProgress(null), "等待同步", "missing sync progress waits for synchronization");
assertEqual(readableSyncProgress(progress), "details：8/20，失败详情 1", "sync progress includes phase and count");
assertEqual(
  readableSyncProgress({ ...progress, phase: "保存数据", processed: 0, total: 0 }),
  "保存数据",
  "zero-total progress displays only its phase",
);
assertEqual(
  productErrorMessage("TypeError: Cannot read properties of undefined (reading 'invoke')"),
  "TypeError: Cannot read properties of undefined (reading 'invoke')",
  "plain invoke errors remain unchanged",
);
assertEqual(
  productErrorMessage(new Error("合成视觉验收错误：排行请求失败，请重新加载。")),
  "Error: 合成视觉验收错误：排行请求失败，请重新加载。",
  "Error inputs retain their native prefix",
);
assertEqual(
  productErrorMessage(new Error("network request failed")),
  "Error: network request failed",
  "plain English errors remain unchanged",
);
assertEqual(
  productErrorMessage("upstream rejected request; token=synthetic-test-fragment"),
  "upstream rejected request; token=synthetic-test-fragment",
  "token-like runtime error fragments remain unchanged",
);
assertEqual(
  productErrorMessage(null),
  "请求失败，请稍后重新加载。",
  "null errors use the stable product fallback",
);
assertEqual(
  productErrorMessage(undefined),
  "请求失败，请稍后重新加载。",
  "undefined errors use the stable product fallback",
);
assertEqual(
  productErrorMessage(""),
  "请求失败，请稍后重新加载。",
  "empty errors use the stable product fallback",
);
assertEqual(
  /function runAddAccount\(\)\s*\{\s*if \(!hasDesktopBackend\(\)\) \{\s*setError\(browserPreviewLoginHint\);\s*return Promise\.resolve\(false\);\s*\}[\s\S]*?invoke: addAccount/.test(onboardingSource),
  true,
  "add-account browser preview selects its hint before invoking the backend",
);
assertEqual(
  /function runReloginAccount\(account: AccountSummary\)\s*\{\s*if \(!hasDesktopBackend\(\)\) \{\s*setError\(browserPreviewLoginHint\);\s*return Promise\.resolve\(false\);\s*\}[\s\S]*?invoke: \(\) => reloginAccount\(account\.accountId\)/.test(onboardingSource),
  true,
  "relogin browser preview selects its hint before invoking the backend",
);
assertEqual(hasDesktopBackend({}), false, "plain browser has no desktop backend");
assertEqual(hasDesktopBackend({ __TAURI_INTERNALS__: {} }), true, "tauri internals indicate desktop backend");
