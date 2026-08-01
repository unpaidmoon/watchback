import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  BadgeInfo,
  CheckCircle2,
  FileText,
  FolderOpen,
  Github,
  MessageSquareText,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Ref } from "react";
import {
  addAccount,
  cancelDashenLogin,
  getOnboardingStatus,
  getUserSettings,
  getSeasonSyncStatus,
  getPlayerProfile,
  getPlayerGameAction,
  getLoginTargetStatus,
  listAccounts,
  removeAccount,
  openSupportDirectory,
  openProductFeedback,
  openProjectRepository,
  recoverLoginTarget,
  retrySeasonBackgroundSync,
  refreshProfileCard,
  switchAccount,
  syncSeason,
  saveUserSettings,
} from "../shared/api";
import type { LoginTargetRecoveryAction } from "../shared/api";
import { AccountManager } from "../features/command-center/AccountManager";
import { DeepSeekSettings } from "../features/command-center/DeepSeekSettings";
import { QrAccountLoginDialog } from "../features/command-center/QrAccountLoginDialog";
import { ProfileGameActionDialog, ProfileSummaryCard } from "../features/profile-card/ProfileSummaryCard";
import {
  loadDashboardRequest,
  runAddAccountOperation,
  runRemoveAccountOperation,
  runSwitchAccountOperation,
} from "../features/command-center/accountOperationIsolation";
import type { BusyAccountAction, DashboardSnapshot } from "../features/command-center/accountOperationIsolation";
import { summarizeCommandCenterSeasons } from "../features/command-center/commandCenterTotals";
import { DataFact, formatTimestamp } from "../features/command-center/SeasonHealthPanel";
import { resolveCommandCenterSeason, SeasonSyncWorkbench } from "../features/command-center/SeasonSyncWorkbench";
import { formatSyncNotice, hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import { clearAccountScopedFrontendCaches } from "../shared/accountScopedFrontendCaches";
import { isCurrentSeasonEvent } from "../shared/seasonSyncEvents";
import { PageHeader } from "../shared/page-shell";
import type { AccountManagerStatus, AccountSummary, CredentialLoginProgress, LoginTargetStatus, OnboardingStatus, PlayerGameAction, PlayerProfile, SeasonBackgroundEvent, SeasonBackgroundStatus, SyncMode, SyncProgress, UserSettings } from "../shared/types";

type TargetAction = LoginTargetRecoveryAction;
type TargetRecoveryPanelProps = {
  targetStatus: LoginTargetStatus | null;
  fallbackMessage: string;
  targetAction: TargetAction | null;
  disabled: boolean;
  primaryActionRef: Ref<HTMLButtonElement>;
  onConfirmSwitch: () => void;
  onKeepCurrent: () => void;
  onRetryBinding: () => void;
  onClear: () => void;
};
const APP_VERSION = "0.4.0";
const browserPreviewLoginHint = "当前浏览器预览无法连接桌面后端，请在 Tauri 应用中使用登录和同步功能。";

let dashboardCache: DashboardSnapshot | null = null;

export function normalizeLoginTargetName(value: string) {
  return value.trim().replace(/＃/g, "#").toLowerCase();
}

export function isCurrentDashboardRequest(requestGeneration: number, currentGeneration: number, mounted: boolean) {
  return mounted && requestGeneration === currentGeneration;
}

export function TargetRecoveryPanel({
  targetStatus,
  fallbackMessage,
  targetAction,
  disabled,
  primaryActionRef,
  onConfirmSwitch,
  onKeepCurrent,
  onRetryBinding,
  onClear,
}: TargetRecoveryPanelProps) {
  const hasCurrentTarget = Boolean(targetStatus?.currentBnet);
  const showDetectedLogin = Boolean(
    targetStatus?.currentBnet &&
    targetStatus.detectedBnet &&
    normalizeLoginTargetName(targetStatus.currentBnet) !== normalizeLoginTargetName(targetStatus.detectedBnet),
  );
  const actionPending = targetAction !== null;

  return (
    <section
      className="syncTargetStrip"
      aria-labelledby="sync-target-title"
      aria-describedby="sync-target-message"
      aria-busy={actionPending}
    >
      <div className="phase8TargetPlayerSummary">
        <span className="phase6Eyebrow" id="sync-target-title">当前目标玩家</span>
        <strong>{targetStatus?.currentBnet ?? targetStatus?.detectedBnet ?? "尚未识别账号"}</strong>
        {showDetectedLogin && (
          <div className="syncTargetComparison" aria-label="当前目标与检测到的登录账号">
            <span>当前目标 <strong>{targetStatus?.currentBnet}</strong></span>
            <span>检测到的登录账号 <strong>{targetStatus?.detectedBnet}</strong></span>
          </div>
        )}
        <p id="sync-target-message" aria-live="polite">
          {targetStatus?.message ?? fallbackMessage}
        </p>
      </div>
      <div className="syncTargetDecision" role="group" aria-label="目标玩家操作">
        {!targetStatus ? <span className="warn" role="status">正在确认目标状态…</span> : null}
        {targetStatus?.state === "ready" ? <span className="good" role="status">当前目标已确认，可同步</span> : null}
        {targetStatus?.state === "switchRequired" ? (
          <span className="warn" role="status">检测到其他登录账号，请选择目标</span>
        ) : null}
        {targetStatus?.state === "bindFailed" ? <span className="warn" role="status">目标绑定失败</span> : null}
        {targetStatus && !targetStatus.syncAllowed && targetStatus.state !== "switchRequired" && targetStatus.state !== "bindFailed" ? (
          <span className="warn" role="status">同步前需要处理目标状态</span>
        ) : null}
        {targetStatus?.state === "switchRequired" ? (
          <>
            <button
              ref={primaryActionRef}
              className="primaryButton"
              type="button"
              onClick={onConfirmSwitch}
              disabled={disabled || !targetStatus.canConfirmSwitch}
            >
              {targetAction === "confirm" ? "切换中…" : "切换到此账号"}
            </button>
            <button className="ghostButton" type="button" onClick={onKeepCurrent} disabled={disabled}>
              {targetAction === "keep" ? "保留中…" : "保留原目标"}
            </button>
          </>
        ) : null}
        {targetStatus?.state === "bindFailed" && targetStatus.canRetry ? (
          <button
            ref={primaryActionRef}
            className="primaryButton"
            type="button"
            onClick={onRetryBinding}
            disabled={disabled}
          >
            {targetAction === "retry" ? "重新绑定中…" : "重新绑定"}
          </button>
        ) : null}
        <button className="ghostButton" type="button" onClick={onClear} disabled={!hasCurrentTarget || disabled}>
          {targetAction === "clear" ? "清除中…" : "清除"}
        </button>
      </div>
    </section>
  );
}

export function OnboardingPage() {
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [accountStatus, setAccountStatus] = useState<AccountManagerStatus | null>(null);
  const [accountContextInvalidated, setAccountContextInvalidated] = useState(false);
  const [profile, setProfile] = useState<PlayerProfile | null>(null);
  const [targetStatus, setTargetStatus] = useState<LoginTargetStatus | null>(null);
  const [selectedSeason, setSelectedSeason] = useState<number | null>(null);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncNotice, setSyncNotice] = useState<string | null>(null);
  const [backgroundStatuses, setBackgroundStatuses] = useState<Record<number, SeasonBackgroundStatus>>({});
  const [loginProgress, setLoginProgress] = useState<CredentialLoginProgress | null>(null);
  const [syncingSeason, setSyncingSeason] = useState<number | null>(null);
  const [syncingMode, setSyncingMode] = useState<SyncMode | null>(null);
  const [userSettings, setUserSettings] = useState<UserSettings | null>(null);
  const [autoUpdateSaving, setAutoUpdateSaving] = useState(false);
  const [busyAccountAction, setBusyAccountAction] = useState<BusyAccountAction | null>(null);
  const [cancellingLogin, setCancellingLogin] = useState(false);
  const [qrLoginDialogOpen, setQrLoginDialogOpen] = useState(false);
  const [versionDialogOpen, setVersionDialogOpen] = useState(false);
  const [gameActionDialogOpen, setGameActionDialogOpen] = useState(false);
  const [gameAction, setGameAction] = useState<PlayerGameAction | null>(null);
  const [gameActionLoading, setGameActionLoading] = useState(false);
  const [openingSupportDirectory, setOpeningSupportDirectory] = useState<"logs" | "data" | null>(null);
  const [openingProjectRepository, setOpeningProjectRepository] = useState(false);
  const [openingProductFeedback, setOpeningProductFeedback] = useState(false);
  const [targetAction, setTargetAction] = useState<TargetAction | null>(null);
  const [profileRefreshing, setProfileRefreshing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const closeVersionDialog = useCallback(() => setVersionDialogOpen(false), []);
  const closeGameActionDialog = useCallback(() => setGameActionDialogOpen(false), []);
  const mountedRef = useRef(false);
  const dashboardRequestGenerationRef = useRef(0);
  const accountOperationLockRef = useRef(false);
  const cancelLoginLockRef = useRef(false);
  const primaryTargetActionRef = useRef<HTMLButtonElement | null>(null);
  const previousPrimaryTargetStateRef = useRef<string | null>(null);
  const activeAccountScopeRef = useRef<string | null>(null);
  const seasonRefreshTimersRef = useRef<Record<number, ReturnType<typeof setTimeout>>>({});

  async function openSettingsDirectory(kind: "logs" | "data") {
    if (openingSupportDirectory) return;
    if (!hasDesktopBackend()) {
      setNotice("请在安装版 Watchback 中打开该路径");
      return;
    }
    setOpeningSupportDirectory(kind);
    setError(null);
    try {
      await openSupportDirectory(kind);
    } catch (err) {
      setError(`打开${kind === "logs" ? "日志文件" : "数据存放路径"}失败：${productErrorMessage(err)}`);
    } finally {
      setOpeningSupportDirectory(null);
    }
  }

  async function openGithubRepository() {
    if (openingProjectRepository) return;
    if (!hasDesktopBackend()) {
      setNotice("请在安装版 Watchback 中打开 Github 地址");
      return;
    }
    setOpeningProjectRepository(true);
    setError(null);
    try {
      await openProjectRepository();
    } catch (err) {
      setError(`打开 Github 地址失败：${productErrorMessage(err)}`);
    } finally {
      setOpeningProjectRepository(false);
    }
  }

  async function openFeedbackCenter() {
    if (openingProductFeedback) return;
    if (!hasDesktopBackend()) {
      setNotice("请在安装版 Watchback 中打开意见反馈");
      return;
    }
    setOpeningProductFeedback(true);
    setError(null);
    try {
      await openProductFeedback();
    } catch (err) {
      setError(`打开意见反馈失败：${productErrorMessage(err)}`);
    } finally {
      setOpeningProductFeedback(false);
    }
  }

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      dashboardRequestGenerationRef.current += 1;
      Object.values(seasonRefreshTimersRef.current).forEach(clearTimeout);
    };
  }, []);

  useEffect(() => {
    activeAccountScopeRef.current = accountStatus?.activeAccountId ?? null;
  }, [accountStatus?.activeAccountId]);

  useEffect(() => {
    if (!hasDesktopBackend()) {
      return;
    }

    let disposed = false;
    let unlistenSync: (() => void) | undefined;
    let unlistenLogin: (() => void) | undefined;
    const backgroundUnlisteners: Array<() => void> = [];
    void listen<SyncProgress>("sync-progress", (event) => {
      if (!disposed) {
        setSyncProgress(event.payload);
      }
    }).then((handler) => {
      if (disposed) {
        handler();
        return;
      }
      unlistenSync = handler;
    });
    void listen<CredentialLoginProgress>("credential-login-progress", (event) => {
      if (!disposed) {
        setLoginProgress(event.payload);
      }
    }).then((handler) => {
      if (disposed) {
        handler();
        return;
      }
      unlistenLogin = handler;
    });
    for (const eventName of [
      "season-background-progress",
      "season-background-completed",
    ]) {
      void listen<SeasonBackgroundEvent>(eventName, (event) => {
        if (disposed || !isCurrentSeasonEvent(event.payload, activeAccountScopeRef.current)) {
          return;
        }
        const nextStatus: SeasonBackgroundStatus = {
          season: event.payload.season,
          phase: event.payload.phase,
          status: event.payload.status,
          pendingDetails: event.payload.pendingDetails,
          retryable: event.payload.status === "failed",
          updatedAt: new Date().toISOString(),
        };
        setBackgroundStatuses((current) => ({ ...current, [event.payload.season]: nextStatus }));
      }).then((handler) => {
        if (disposed) {
          handler();
          return;
        }
        backgroundUnlisteners.push(handler);
      });
    }
    return () => {
      disposed = true;
      unlistenSync?.();
      unlistenLogin?.();
      backgroundUnlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    void loadDashboard();
    if (hasDesktopBackend()) {
      void getUserSettings().then(setUserSettings).catch(() => undefined);
    }
  }, []);

  useEffect(() => {
    setGameAction(profile?.gameAction ?? null);
    setGameActionDialogOpen(false);
  }, [accountStatus?.activeAccountId, profile?.gameAction]);

  useEffect(() => {
    const seasons = status?.seasonStatuses.map((item) => item.season) ?? [];
    if (!accountStatus?.activeAccountId || seasons.length === 0 || !hasDesktopBackend()) {
      setBackgroundStatuses({});
      return;
    }
    let disposed = false;
    void Promise.all(seasons.map((season) => getSeasonSyncStatus(season))).then((items) => {
      if (!disposed) {
        setBackgroundStatuses(Object.fromEntries(items.map((item) => [item.season, item])));
      }
    }).catch(() => undefined);
    return () => {
      disposed = true;
    };
  }, [accountStatus?.activeAccountId, status?.seasonStatuses]);

  async function loadDashboard(rethrow = false) {
    const requestGeneration = ++dashboardRequestGenerationRef.current;
    if (mountedRef.current) {
      setError(null);
    }
    try {
      const result = await loadDashboardRequest({
        requestGeneration,
        isCurrent: (generation) => isCurrentDashboardRequest(
          generation,
          dashboardRequestGenerationRef.current,
          mountedRef.current,
        ),
        cachedSnapshot: dashboardCache,
        listAccounts,
        loadAccountScope: async () => {
          const [nextStatus, nextProfile, nextTargetStatus] = await Promise.all([
            getOnboardingStatus(),
            getPlayerProfile(),
            getLoginTargetStatus(),
          ]);
          return { status: nextStatus, profile: nextProfile, targetStatus: nextTargetStatus };
        },
        onAccountStatus: (nextAccountStatus, reusableCache) => {
          setAccountStatus(nextAccountStatus);
          if (reusableCache) {
            applyDashboardSnapshot(reusableCache);
          } else {
            dashboardCache = null;
            setStatus(null);
            setProfile(null);
            setTargetStatus(null);
          }
        },
      });
      if (result.kind === "stale") {
        return;
      }
      dashboardCache = result.snapshot;
      applyDashboardSnapshot(result.snapshot);
    } catch (err) {
      if (isCurrentDashboardRequest(requestGeneration, dashboardRequestGenerationRef.current, mountedRef.current)) {
        if (!rethrow) {
          setError(productErrorMessage(err));
        }
      }
      if (rethrow) {
        throw err;
      }
    }
  }

  async function refreshSeasonStatus(season: number) {
    try {
      const nextStatus = await getOnboardingStatus(season);
      if (!mountedRef.current) return;
      setStatus((current) => current ? {
        ...current,
        seasonStatuses: nextStatus.seasonStatuses,
      } : nextStatus);
    } catch {
      // 后台刷新失败不阻断已经可查询的本地对局。
    }
  }

  function scheduleSeasonStatusRefresh(season: number) {
    clearTimeout(seasonRefreshTimersRef.current[season]);
    seasonRefreshTimersRef.current[season] = setTimeout(() => {
      delete seasonRefreshTimersRef.current[season];
      void refreshSeasonStatus(season);
    }, 250);
  }

  function applyDashboardSnapshot(snapshot: DashboardSnapshot) {
    setAccountContextInvalidated(false);
    setAccountStatus(snapshot.accountStatus);
    setStatus(snapshot.status);
    setProfile(snapshot.profile);
    setTargetStatus(snapshot.targetStatus);
    setSelectedSeason((current) => snapshot.status
      ? resolveCommandCenterSeason(
          current ?? snapshot.status.selectedSeason ?? snapshot.status.defaultSeason,
          snapshot.status.minimumSyncSeason,
          snapshot.status.defaultSeason,
        )
      : null);
  }

  function acquireOperationLock() {
    if (accountOperationLockRef.current) {
      return false;
    }
    accountOperationLockRef.current = true;
    return true;
  }

  function releaseOperationLock() {
    accountOperationLockRef.current = false;
  }

  function invalidateDashboardCache() {
    dashboardCache = null;
    dashboardRequestGenerationRef.current += 1;
  }

  function beginAccountOperation() {
    if (!acquireOperationLock()) {
      return false;
    }
    invalidateDashboardCache();
    return true;
  }

  function failClosedAccountContext() {
    invalidateDashboardCache();
    setAccountContextInvalidated(true);
    setStatus(null);
    setProfile(null);
    setTargetStatus(null);
    setSelectedSeason(null);
    setSyncProgress(null);
    setSyncError(null);
    setSyncNotice(null);
    setSyncingSeason(null);
    setSyncingMode(null);
    setAccountStatus((current) => current ? {
      ...current,
      activeAccountId: null,
      accounts: current.accounts.map((account) => ({ ...account, isActive: false })),
      message: "正在重新加载账号状态。",
    } : current);
  }

  function accountOperationLifecycle() {
    return {
      begin: beginAccountOperation,
      release: releaseOperationLock,
      setBusy: (action: BusyAccountAction | null) => {
        if (mountedRef.current) {
          if (action?.kind === "add") {
            setLoginProgress(null);
          }
          setBusyAccountAction(action);
        }
      },
      setNotice: (message: string | null) => {
        if (mountedRef.current) setNotice(message);
      },
      setError: (message: string | null) => {
        if (mountedRef.current) setError(message);
      },
      formatError: productErrorMessage,
      refreshAccounts: loadDashboard,
      clearCache: clearAccountScopedFrontendCaches,
      failClosed: failClosedAccountContext,
      refreshAfterMutation: () => loadDashboard(true),
    };
  }

  function runAddAccount() {
    if (!hasDesktopBackend()) {
      setError(browserPreviewLoginHint);
      return Promise.resolve(false);
    }
    return runAddAccountOperation({
      ...accountOperationLifecycle(),
      invoke: addAccount,
    });
  }

  function openQrAccountLogin() {
    if (!hasDesktopBackend()) {
      setError(browserPreviewLoginHint);
      return;
    }
    setError(null);
    setNotice(null);
    setQrLoginDialogOpen(true);
  }

  async function completeQrAccountLogin(message: string | null) {
    try {
      await loadDashboard();
      if (mountedRef.current) {
        setNotice(message ?? "\u8d26\u53f7\u6dfb\u52a0\u6210\u529f\u3002");
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`\u8d26\u53f7\u5df2\u6dfb\u52a0\uff0c\u4f46\u5237\u65b0\u8d26\u53f7\u5217\u8868\u5931\u8d25\uff1a${productErrorMessage(err)}\u3002\u8bf7\u91cd\u8bd5\u6216\u91cd\u542f\u3002`);
      }
    }
  }

  function runSwitchAccount(account: AccountSummary) {
    return runSwitchAccountOperation(account, {
      ...accountOperationLifecycle(),
      invoke: () => switchAccount(account.accountId),
      reload: () => window.location.reload(),
    });
  }

  function runRemoveAccount(account: AccountSummary) {
    return runRemoveAccountOperation(account, {
      ...accountOperationLifecycle(),
      invoke: () => removeAccount(account.accountId),
      reload: () => window.location.reload(),
    });
  }

  async function cancelLogin() {
    if (cancelLoginLockRef.current) {
      return;
    }
    cancelLoginLockRef.current = true;
    setCancellingLogin(true);
    setError(null);
    try {
      await cancelDashenLogin();
      if (mountedRef.current) {
        setLoginProgress({ phase: "cancelled", message: "已取消登录等待。" });
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`取消登录失败：${productErrorMessage(err)}。请重试。`);
      }
    } finally {
      if (mountedRef.current) {
        setCancellingLogin(false);
      }
      cancelLoginLockRef.current = false;
    }
  }

  async function runSeasonSync(season: number, mode: SyncMode) {
    if (!acquireOperationLock()) {
      setSyncError("当前正在处理账号操作，请完成后再同步。");
      return;
    }
    dashboardRequestGenerationRef.current += 1;
    setSyncingSeason(season);
    setSyncingMode(mode);
    setNotice(null);
    setError(null);
    setSyncNotice(null);
    setSyncError(null);
    setSyncProgress(null);
    try {
      const summary = await syncSeason(season, mode);
      if (summary.kind === "full") {
        await loadDashboard();
      } else if (summary.dataChanged) {
        scheduleSeasonStatusRefresh(summary.season);
      }
      if (summary.kind === "quickList") {
        setBackgroundStatuses((current) => ({
          ...current,
          [season]: {
            season,
            phase: summary.backgroundStatus === "queued" ? "continuePages" : "completed",
            status: summary.backgroundStatus,
            pendingDetails: summary.pendingDetails,
            retryable: false,
            updatedAt: summary.checkedAt,
          },
        }));
      }
      setSyncNotice(formatSyncNotice(season, summary));
    } catch (err) {
      setSyncError(productErrorMessage(err));
    } finally {
      setSyncingSeason(null);
      setSyncingMode(null);
      releaseOperationLock();
    }
  }

  async function refreshCurrentProfile() {
    if (profileRefreshing || accountOperationBusy || syncingSeason !== null) return;
    setProfileRefreshing(true);
    setError(null);
    setNotice(null);
    try {
      const nextProfile = await refreshProfileCard();
      if (!mountedRef.current) return;
      setProfile(nextProfile);
      if (dashboardCache) {
        dashboardCache = { ...dashboardCache, profile: nextProfile };
      }
      setNotice("账号资料已刷新。");
    } catch (err) {
      if (mountedRef.current) {
        setError(`刷新账号资料失败：${productErrorMessage(err)}。请稍后重试。`);
      }
    } finally {
      if (mountedRef.current) setProfileRefreshing(false);
    }
  }

  async function openGameAction() {
    if (gameActionLoading || accountOperationBusy || syncingSeason !== null) return;
    setGameActionLoading(true);
    setError(null);
    try {
      const nextGameAction = await getPlayerGameAction();
      if (!mountedRef.current) return;
      setGameAction(nextGameAction);
      setGameActionDialogOpen(true);
    } catch (err) {
      if (mountedRef.current) {
        setError(`读取本周表现失败：${productErrorMessage(err)}。请稍后重试。`);
      }
    } finally {
      if (mountedRef.current) setGameActionLoading(false);
    }
  }

  async function changeAutoUpdate(enabled: boolean) {
    if (autoUpdateSaving) return;
    setAutoUpdateSaving(true);
    setSyncError(null);
    let previous = userSettings;
    try {
      previous ??= await getUserSettings();
      setUserSettings({ ...previous, autoUpdateSeason: enabled });
      const saved = await saveUserSettings({ ...previous, autoUpdateSeason: enabled });
      if (mountedRef.current) setUserSettings(saved);
    } catch (err) {
      if (mountedRef.current) {
        setUserSettings(previous);
        setSyncError(`保存自动更新设置失败：${productErrorMessage(err)}`);
      }
    } finally {
      if (mountedRef.current) setAutoUpdateSaving(false);
    }
  }

  async function retryBackgroundSync(season: number) {
    try {
      const queued = await retrySeasonBackgroundSync(season);
      if (queued) {
        const next = await getSeasonSyncStatus(season);
        setBackgroundStatuses((current) => ({ ...current, [season]: next }));
      }
    } catch (err) {
      setSyncError(productErrorMessage(err));
    }
  }

  function selectSyncSeason(season: number) {
    setSyncError(null);
    setSyncNotice(null);
    setSelectedSeason(season);
  }

  async function runTargetRecovery(action: TargetAction) {
    if (!beginAccountOperation()) {
      return;
    }
    setTargetAction(action);
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await recoverLoginTarget(action);
      if (mountedRef.current) {
        setTargetStatus(nextTarget);
        setNotice(nextTarget.message);
      }
    } catch (err) {
      if (mountedRef.current) {
        const actionLabel = {
          confirm: "切换目标",
          keep: "保留原目标",
          retry: "重新绑定",
          clear: "清除目标",
        }[action];
        const recoveryHint = action === "clear" ? "请重试。" : "请重试或通过添加账号更新登录。";
        setError(`${actionLabel}失败：${productErrorMessage(err)}。${recoveryHint}`);
      }
    } finally {
      if (mountedRef.current) {
        setTargetAction(null);
      }
      releaseOperationLock();
    }
  }

  const credentialReady = status?.credentialStatus.state === "ready";
  const accountOperationBusy = busyAccountAction !== null || targetAction !== null || qrLoginDialogOpen;
  const activeAccount = accountStatus?.accounts.find((account) => account.isActive) ?? null;
  const primaryTargetState = targetStatus && ["switchRequired", "bindFailed", "unidentified", "verificationRequired", "mismatchKept"].includes(targetStatus.state)
    ? targetStatus.state
    : null;
  const warnings = status?.health.warnings ?? [];
  const displayName = profile?.playerId ?? activeAccount?.battleTag ?? "未登录玩家";
  const lastSync = formatTimestamp(status?.localData.lastSyncedOrImportedAt);
  const canUseData = Boolean(status?.canEnterReview);
  const {
    matchCount: localMatches,
    detailCount: detailMatches,
    coveragePercent,
  } = summarizeCommandCenterSeasons(status?.seasonStatuses ?? []);

  useEffect(() => {
    if (!primaryTargetState) {
      previousPrimaryTargetStateRef.current = null;
      return;
    }
    if (accountOperationBusy || syncingSeason !== null || previousPrimaryTargetStateRef.current === primaryTargetState) {
      return;
    }
    const button = primaryTargetActionRef.current;
    if (!button || button.disabled) {
      return;
    }
    button.focus();
    if (document.activeElement === button) {
      previousPrimaryTargetStateRef.current = primaryTargetState;
    }
  }, [primaryTargetState, accountOperationBusy, syncingSeason]);

  return (
    <section className="workspace onboardingWorkspace phase6CommandCenter">
      <PageHeader
        title="指挥台"
        description=""
      />
      {(error || notice) && (
        <div
          className={`errorBanner inline phase6RecoverableError ${notice && !error ? "good" : ""}`}
          role={error ? "alert" : "status"}
          aria-live={error ? "assertive" : "polite"}
        >
          {error ? <AlertCircle size={18} /> : <CheckCircle2 size={18} />}
          <span>{error ?? notice}</span>
        </div>
      )}

      {!accountContextInvalidated && accountStatus?.activeAccountId ? <div className="commandCenterMetricsRow" aria-label="指挥台状态摘要">
        <DataFact label="登录状态" value={credentialReady ? "已登录" : "待登录"} tone={credentialReady ? "good" : "warn"} />
        <DataFact label="本地对局" value={`${localMatches} 场`} tone={localMatches > 0 ? "good" : "warn"} />
        <DataFact label="覆盖率" value={`${coveragePercent}%`} tone={coveragePercent >= 80 ? "good" : "warn"} />
        <DataFact label="最近同步" value={lastSync} tone={lastSync === "-" ? "warn" : "neutral"} />
      </div> : null}

      <div className="commandCenterAccountRegion">
        {accountStatus === null || accountContextInvalidated ? (
          <div className="seasonSyncWorkbenchLoading" role="status">
            {accountContextInvalidated ? "账号状态已更新，正在重新加载…" : "正在加载账号信息…"}
          </div>
        ) : accountStatus.activeAccountId ? <>
          <div className="commandCenterMainGrid" id="local-data" tabIndex={-1}>
            <section className="commandCenterSyncPanel" aria-labelledby="command-center-sync-title">
              <div className="commandCenterSectionHeading">
                <div>
                  <span className="phase6Eyebrow">主要操作</span>
                  <h2 id="command-center-sync-title">赛季数据与同步</h2>
                </div>
                <strong>{readableSyncProgress(syncProgress)}</strong>
              </div>
              <TargetRecoveryPanel
                targetStatus={targetStatus}
                fallbackMessage={status?.credentialStatus.message ?? "请登录网易大神并自动识别当前账号"}
                targetAction={targetAction}
                disabled={accountOperationBusy || syncingSeason !== null}
                primaryActionRef={primaryTargetActionRef}
                onConfirmSwitch={() => void runTargetRecovery("confirm")}
                onKeepCurrent={() => void runTargetRecovery("keep")}
                onRetryBinding={() => void runTargetRecovery("retry")}
                onClear={() => void runTargetRecovery("clear")}
              />
              {status && selectedSeason !== null ? (
                <SeasonSyncWorkbench
                  selectedSeason={selectedSeason}
                  minimumSyncSeason={status.minimumSyncSeason}
                  seasonStatuses={status.seasonStatuses}
                  credentialReady={credentialReady}
                  targetSyncAllowed={Boolean(targetStatus?.syncAllowed)}
                  accountOperationBusy={accountOperationBusy}
                  syncingSeason={syncingSeason}
                  syncingMode={syncingMode}
                  progress={syncProgress}
                  syncError={syncError}
                  syncNotice={syncNotice}
                  autoUpdateEnabled={userSettings?.autoUpdateSeason ?? true}
                  autoUpdateSaving={autoUpdateSaving}
                  backgroundStatuses={backgroundStatuses}
                  onSelectSeason={selectSyncSeason}
                  onFullFetch={(season) => void runSeasonSync(season, "fullFetch")}
                  onQuickUpdate={(season) => void runSeasonSync(season, "quickUpdate")}
                  onRetryBackground={(season) => void retryBackgroundSync(season)}
                  onAutoUpdateChange={(enabled) => void changeAutoUpdate(enabled)}
                />
              ) : (
                <div className="seasonSyncWorkbenchLoading" role="status">正在加载赛季数据…</div>
              )}
          </section>

          <aside className="commandCenterAccountPanel" aria-labelledby="command-center-account-title">
            <ProfileSummaryCard
              nameHeadingId="command-center-account-title"
              profile={{
                name: displayName,
                title: profile?.title ?? null,
                titleIconUrl: profile?.titleIconUrl ?? null,
                avatarUrl: profile?.avatarUrl ?? null,
                endorsementLevel: profile?.endorsementLevel ?? null,
                playTimeHours: profile?.playTimeHours ?? null,
                gameAction,
              }}
              avatarAction={{
                onClick: () => void refreshCurrentProfile(),
                busy: profileRefreshing,
                disabled: profileRefreshing || accountOperationBusy || syncingSeason !== null,
              }}
              gameActionLoading={gameActionLoading}
              onGameActionClick={() => void openGameAction()}
            />
            <section className="phase6CoverageMeter phase6DataCoverageCard" aria-label="数据覆盖度">
              <div className="phase6CoveragePrimary">
                <div><span className="phase6Eyebrow">数据覆盖度</span><strong>{coveragePercent}%</strong></div>
              </div>
              <div className="phase6CoverageTrack" aria-hidden="true"><span style={{ width: `${coveragePercent}%` }} /></div>
              <div className="phase6CoverageFooter"><span>{canUseData ? "数据已就绪" : "等待数据"}</span><strong>{detailMatches}/{localMatches} 场详情</strong></div>
            </section>
            <div className="phase6DataHealthGrid" aria-label="数据检查">
              <HealthItem label={credentialReady ? "登录凭证可用" : "登录凭证待登录"} ok={credentialReady} />
              <HealthItem label={status?.health.readable ? "本地数据库可用" : "本地数据库待检查"} ok={Boolean(status?.health.readable)} />
              <HealthItem label={localMatches > 0 ? "本地赛季有数据" : "本地赛季等待数据"} ok={localMatches > 0} />
              <HealthItem label="没有待处理失败" ok={!warnings.length} />
            </div>
          </aside>
        </div>

        </> : (
          <section className="commandCenterAccountEmpty" id="local-data" tabIndex={-1} aria-labelledby="command-center-empty-title">
            <h2 id="command-center-empty-title">尚未添加账号</h2>
            <p>添加账号后即可同步和查看本地数据。</p>
          </section>
        )}

        <div className="commandCenterUtilityGrid">
          {accountStatus ? <div className="accountManagerPanel">
            {busyAccountAction?.kind === "add" && (
              <div className="accountManagerLoginProgress" role="status" aria-live="polite">
                <span>{loginProgress?.message ?? "正在等待浏览器登录…"}</span>
                <button className="ghostButton" type="button" onClick={cancelLogin} disabled={cancellingLogin}>
                  <X size={18} />
                  {cancellingLogin ? "取消中…" : "取消"}
                </button>
              </div>
            )}
            <AccountManager
              accounts={accountStatus?.accounts ?? []}
              busyAction={qrLoginDialogOpen ? { kind: "add" } : busyAccountAction}
              onAdd={openQrAccountLogin}
              onSwitch={runSwitchAccount}
              onRemove={runRemoveAccount}
            />
          </div> : null}
          <section className="commandCenterTools" aria-labelledby="command-center-tools-title">
            <div><span className="phase6Eyebrow">应用工具</span><h2 id="command-center-tools-title">本地应用与支持</h2></div>
            <DeepSeekSettings />
            <div className="phase6SettingsLauncherGrid syncSecondaryActions">
              <SettingsPlaceholder icon={<BadgeInfo size={17} />} label="版本号" onClick={() => setVersionDialogOpen(true)} />
              <SettingsPlaceholder icon={<FileText size={17} />} label="日志文件" onClick={() => void openSettingsDirectory("logs")} disabled={openingSupportDirectory !== null} />
              <SettingsPlaceholder icon={<FolderOpen size={17} />} label="数据存放路径" onClick={() => void openSettingsDirectory("data")} disabled={openingSupportDirectory !== null} />
              <SettingsPlaceholder icon={<Github size={17} />} label="Github地址" onClick={() => void openGithubRepository()} disabled={openingProjectRepository} />
              <SettingsPlaceholder icon={<MessageSquareText size={17} />} label="意见反馈" onClick={() => void openFeedbackCenter()} disabled={openingProductFeedback} />
            </div>
          </section>
        </div>
        <QrAccountLoginDialog
          open={qrLoginDialogOpen}
          onCancel={() => setQrLoginDialogOpen(false)}
          onCompleted={completeQrAccountLogin}
          onUseExistingLogin={async () => { await runAddAccount(); }}
        />
        <VersionDialog open={versionDialogOpen} onClose={closeVersionDialog} />
        {gameActionDialogOpen && gameAction ? (
          <ProfileGameActionDialog action={gameAction} onClose={closeGameActionDialog} />
        ) : null}
      </div>

    </section>
  );
}


function SettingsPlaceholder({ icon, label, onClick, disabled = false }: { icon: React.ReactNode; label: string; onClick: () => void; disabled?: boolean }) {
  return (
    <button className="ghostButton phase6SettingsPlaceholderAction" type="button" onClick={onClick} disabled={disabled}>
      {icon}
      {label}
    </button>
  );
}

function VersionDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButtonRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="versionDialogBackdrop" role="presentation" onMouseDown={onClose}>
      <section className="versionDialog" role="dialog" aria-modal="true" aria-labelledby="version-dialog-title" onMouseDown={(event) => event.stopPropagation()}>
        <BadgeInfo aria-hidden="true" size={24} />
        <div>
          <h3 id="version-dialog-title">版本号</h3>
          <p className="versionDialogNumber">{APP_VERSION}</p>
        </div>
        <button ref={closeButtonRef} className="iconButton versionDialogClose" type="button" aria-label="关闭版本号弹窗" onClick={onClose}>
          <X aria-hidden="true" size={20} />
        </button>
      </section>
    </div>
  );
}

function HealthItem({ label, ok }: { label: string; ok: boolean }) {
  return <span className={ok ? "good" : "bad"}>{label}</span>;
}
