import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  BadgeInfo,
  CheckCircle2,
  Download,
  FileText,
  FolderOpen,
  Github,
  UserRound,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  addAccount,
  cancelDashenLogin,
  checkAppUpdate,
  confirmLoginTargetSwitch,
  getOnboardingStatus,
  getPlayerProfile,
  getLoginTargetStatus,
  listAccounts,
  clearPlayerTarget,
  keepCurrentPlayerTarget,
  reloginAccount,
  removeAccount,
  retryLoginTargetBinding,
  switchAccount,
  syncSeason,
} from "../shared/api";
import { AccountManager } from "../features/command-center/AccountManager";
import {
  loadDashboardRequest,
  runAddAccountOperation,
  runReloginAccountOperation,
  runRemoveAccountOperation,
  runSwitchAccountOperation,
} from "../features/command-center/accountOperationIsolation";
import type { BusyAccountAction, DashboardSnapshot } from "../features/command-center/accountOperationIsolation";
import { DataFact, formatTimestamp } from "../features/command-center/SeasonHealthPanel";
import { resolveCommandCenterSeason, SeasonSyncWorkbench } from "../features/command-center/SeasonSyncWorkbench";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { formatSyncNotice, hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import { clearAccountScopedFrontendCaches } from "../shared/accountScopedFrontendCaches";
import { clearMatchesPageCache } from "../shared/matchesPageCache";
import { PageHeader } from "../shared/page-shell";
import type { AccountManagerStatus, AccountSummary, CredentialLoginProgress, LoginTargetStatus, OnboardingStatus, PlayerProfile, SyncMode, SyncProgress } from "../shared/types";

type TargetAction = "confirm" | "keep" | "retry" | "clear";

let dashboardCache: DashboardSnapshot | null = null;

export function normalizeLoginTargetName(value: string) {
  return value.trim().replace(/＃/g, "#").toLowerCase();
}

export function isCurrentDashboardRequest(requestGeneration: number, currentGeneration: number, mounted: boolean) {
  return mounted && requestGeneration === currentGeneration;
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
  const [loginProgress, setLoginProgress] = useState<CredentialLoginProgress | null>(null);
  const [syncingSeason, setSyncingSeason] = useState<number | null>(null);
  const [syncingMode, setSyncingMode] = useState<SyncMode | null>(null);
  const [busyAccountAction, setBusyAccountAction] = useState<BusyAccountAction | null>(null);
  const [cancellingLogin, setCancellingLogin] = useState(false);
  const [targetAction, setTargetAction] = useState<TargetAction | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(false);
  const dashboardRequestGenerationRef = useRef(0);
  const accountOperationLockRef = useRef(false);
  const cancelLoginLockRef = useRef(false);
  const primaryTargetActionRef = useRef<HTMLButtonElement | null>(null);
  const previousPrimaryTargetStateRef = useRef<string | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      dashboardRequestGenerationRef.current += 1;
    };
  }, []);

  useEffect(() => {
    if (!hasDesktopBackend()) {
      return;
    }

    let disposed = false;
    let unlistenSync: (() => void) | undefined;
    let unlistenLogin: (() => void) | undefined;
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
    return () => {
      disposed = true;
      unlistenSync?.();
      unlistenLogin?.();
    };
  }, []);

  useEffect(() => {
    void loadDashboard();
  }, []);

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

  function applyDashboardSnapshot(snapshot: DashboardSnapshot) {
    setAccountContextInvalidated(false);
    setAccountStatus(snapshot.accountStatus);
    setStatus(snapshot.status);
    setProfile(snapshot.profile);
    setTargetStatus(snapshot.targetStatus);
    setSelectedSeason((current) => snapshot.status
      ? resolveCommandCenterSeason(
          current ?? snapshot.status.currentSeason,
          snapshot.status.syncableSeasons,
          snapshot.status.currentSeason,
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
          if (action?.kind === "add" || action?.kind === "relogin") {
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
    return runAddAccountOperation({
      ...accountOperationLifecycle(),
      invoke: addAccount,
    });
  }

  function runReloginAccount(account: AccountSummary) {
    return runReloginAccountOperation(account, {
      ...accountOperationLifecycle(),
      invoke: () => reloginAccount(account.accountId),
    });
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
      clearMatchesPageCache();
      await loadDashboard();
      setSyncNotice(formatSyncNotice(season, summary));
    } catch (err) {
      setSyncError(productErrorMessage(err));
    } finally {
      setSyncingSeason(null);
      setSyncingMode(null);
      releaseOperationLock();
    }
  }

  function selectSyncSeason(season: number) {
    setSyncError(null);
    setSyncNotice(null);
    setSelectedSeason(season);
  }

  async function runConfirmTargetSwitch() {
    if (!beginAccountOperation()) {
      return;
    }
    setTargetAction("confirm");
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await confirmLoginTargetSwitch();
      if (mountedRef.current) {
        setTargetStatus(nextTarget);
        setNotice(nextTarget.message);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`切换目标失败：${productErrorMessage(err)}。请重试或重新识别。`);
      }
    } finally {
      if (mountedRef.current) {
        setTargetAction(null);
      }
      releaseOperationLock();
    }
  }

  async function runKeepCurrentTarget() {
    if (!beginAccountOperation()) {
      return;
    }
    setTargetAction("keep");
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await keepCurrentPlayerTarget();
      if (mountedRef.current) {
        setTargetStatus(nextTarget);
        setNotice(nextTarget.message);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`保留原目标失败：${productErrorMessage(err)}。请重试或重新识别。`);
      }
    } finally {
      if (mountedRef.current) {
        setTargetAction(null);
      }
      releaseOperationLock();
    }
  }

  async function runRetryTargetBinding() {
    if (!beginAccountOperation()) {
      return;
    }
    setTargetAction("retry");
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await retryLoginTargetBinding();
      if (mountedRef.current) {
        setTargetStatus(nextTarget);
        setNotice(nextTarget.message);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`重新绑定失败：${productErrorMessage(err)}。请重试或重新识别。`);
      }
    } finally {
      if (mountedRef.current) {
        setTargetAction(null);
      }
      releaseOperationLock();
    }
  }

  async function runClearTarget() {
    if (!beginAccountOperation()) {
      return;
    }
    setTargetAction("clear");
    setNotice(null);
    setError(null);
    try {
      const result = await clearPlayerTarget();
      if (mountedRef.current) {
        setTargetStatus(result.target);
        setNotice(result.target.message);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`清除目标失败：${productErrorMessage(err)}。请重试。`);
      }
    } finally {
      if (mountedRef.current) {
        setTargetAction(null);
      }
      releaseOperationLock();
    }
  }

  async function runUpdateCheck() {
    setCheckingUpdate(true);
    setNotice(null);
    setError(null);
    try {
      const result = await checkAppUpdate();
      setNotice(result.message);
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setCheckingUpdate(false);
    }
  }

  const credentialReady = status?.credentialStatus.state === "ready";
  const accountOperationBusy = busyAccountAction !== null || targetAction !== null;
  const activeAccount = accountStatus?.accounts.find((account) => account.isActive) ?? null;
  const hasCurrentTarget = Boolean(targetStatus?.currentBnet);
  const primaryTargetState = targetStatus && ["switchRequired", "bindFailed", "unidentified", "verificationRequired", "mismatchKept"].includes(targetStatus.state)
    ? targetStatus.state
    : null;
  const showDetectedLogin = Boolean(
    targetStatus?.currentBnet &&
    targetStatus.detectedBnet &&
    normalizeLoginTargetName(targetStatus.currentBnet) !== normalizeLoginTargetName(targetStatus.detectedBnet),
  );
  const warnings = status?.health.warnings ?? [];
  const displayName = activeAccount?.battleTag ?? "未登录玩家";
  const lastSync = formatTimestamp(status?.localData.lastSyncedOrImportedAt);
  const canUseData = Boolean(status?.canEnterReview);
  const presetMatches = status?.localData.presetMatches ?? 0;
  const detailMatches = status?.localData.detailMatches ?? 0;
  const coveragePercent = presetMatches > 0 ? Math.min(100, Math.round((detailMatches / presetMatches) * 100)) : 0;

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
        description="管理当前登录账号、赛季同步与本地数据状态"
        actions={(
          <>
            <button className="ghostButton" type="button" onClick={runUpdateCheck} disabled={checkingUpdate}>
              <Download size={18} />
              {checkingUpdate ? "检查中" : "检查更新"}
            </button>
          </>
        )}
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
        <DataFact label="本地对局" value={`${presetMatches} 场`} tone={presetMatches > 0 ? "good" : "warn"} />
        <DataFact label="覆盖率" value={`${coveragePercent}%`} tone={coveragePercent >= 80 ? "good" : "warn"} />
        <DataFact label="最近同步" value={lastSync} tone={lastSync === "-" ? "warn" : "neutral"} />
      </div> : null}

      <QaRegion id="CMD-PROFILE" className="phase6QaBlockRegion">
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
              <section className="syncTargetStrip" aria-label="当前目标玩家">
                <div className="phase8TargetPlayerSummary">
                  <span className="phase6Eyebrow">当前目标玩家</span>
                  <strong>{targetStatus?.currentBnet ?? targetStatus?.detectedBnet ?? "尚未识别账号"}</strong>
                  {showDetectedLogin && (
                    <div className="syncTargetComparison" aria-label="当前目标与检测到的登录账号">
                      <span>当前目标 <strong>{targetStatus?.currentBnet}</strong></span>
                      <span>检测到的登录账号 <strong>{targetStatus?.detectedBnet}</strong></span>
                    </div>
                  )}
                  <p>{targetStatus?.message ?? status?.credentialStatus.message ?? "请登录网易大神并自动识别当前账号"}</p>
                </div>
                <div className="syncTargetDecision">
                  {targetStatus?.state === "ready" && <span className="good">当前目标已确认，可同步</span>}
                  {targetStatus?.state === "switchRequired" && (
                    <>
                      <button ref={primaryTargetActionRef} className="primaryButton" type="button" onClick={runConfirmTargetSwitch} disabled={accountOperationBusy || syncingSeason !== null || !targetStatus.canConfirmSwitch}>
                        {targetAction === "confirm" ? "切换中…" : "切换到此账号"}
                      </button>
                      <button className="ghostButton" type="button" onClick={runKeepCurrentTarget} disabled={accountOperationBusy || syncingSeason !== null}>
                        {targetAction === "keep" ? "保留中…" : "保留原目标"}
                      </button>
                    </>
                  )}
                  {targetStatus?.state === "bindFailed" && targetStatus.canRetry ? (
                    <button ref={primaryTargetActionRef} className="primaryButton" type="button" onClick={runRetryTargetBinding} disabled={accountOperationBusy || syncingSeason !== null}>
                      {targetAction === "retry" ? "重新绑定中…" : "重新绑定"}
                    </button>
                  ) : targetStatus && ["unidentified", "verificationRequired", "mismatchKept", "bindFailed"].includes(targetStatus.state) ? (
                    <button ref={primaryTargetActionRef} className="primaryButton" type="button" onClick={() => activeAccount && runReloginAccount(activeAccount)} disabled={!activeAccount || accountOperationBusy || syncingSeason !== null}>
                      {busyAccountAction?.kind === "relogin" ? "识别中…" : "重新识别"}
                    </button>
                  ) : null}
                  <button className="ghostButton" type="button" onClick={runClearTarget} disabled={!hasCurrentTarget || accountOperationBusy || syncingSeason !== null}>
                    {targetAction === "clear" ? "清除中…" : "清除"}
                  </button>
                </div>
              </section>
              {status && selectedSeason !== null ? (
                <SeasonSyncWorkbench
                  selectedSeason={selectedSeason}
                  currentSeason={status.currentSeason}
                  syncableSeasons={status?.syncableSeasons ?? []}
                  seasonStatuses={status.seasonStatuses}
                  credentialReady={credentialReady}
                  targetSyncAllowed={Boolean(targetStatus?.syncAllowed)}
                  accountOperationBusy={accountOperationBusy}
                  syncingSeason={syncingSeason}
                  syncingMode={syncingMode}
                  progress={syncProgress}
                  syncError={syncError}
                  syncNotice={syncNotice}
                  onSelectSeason={selectSyncSeason}
                  onFullFetch={(season) => void runSeasonSync(season, "fullFetch")}
                  onQuickUpdate={(season) => void runSeasonSync(season, "quickUpdate")}
                />
              ) : (
                <div className="seasonSyncWorkbenchLoading" role="status">正在加载赛季数据…</div>
              )}
          </section>

          <aside className="commandCenterAccountPanel" aria-labelledby="command-center-account-title">
            <div className="commandCenterIdentity">
              <div className="phase6ProfileAvatar">
                {profile?.avatarUrl ? <img src={profile.avatarUrl} alt="" /> : <UserRound size={32} />}
              </div>
              <div>
                <span className="phase6Eyebrow">当前账号与健康</span>
                <h2 id="command-center-account-title">{displayName}</h2>
                <p>{profile?.title ?? "暂无头衔"}</p>
              </div>
            </div>
            <div className="commandCenterAccountFacts">
              <ProfileInlineStat label="赞赏等级" value={profile?.endorsementLevel ?? "-"} />
              <ProfileInlineStat label="游戏时间" value={profile?.playTimeHours != null ? `${profile.playTimeHours.toFixed(1)} 小时` : "-"} />
            </div>
            <section className="phase6CoverageMeter phase6DataCoverageCard" aria-label="数据覆盖度">
              <div className="phase6CoveragePrimary">
                <div><span className="phase6Eyebrow">数据覆盖度</span><strong>{coveragePercent}%</strong></div>
              </div>
              <div className="phase6CoverageTrack" aria-hidden="true"><span style={{ width: `${coveragePercent}%` }} /></div>
              <div className="phase6CoverageFooter"><span>{canUseData ? "数据已就绪" : "等待数据"}</span><strong>{detailMatches}/{presetMatches} 场详情</strong></div>
            </section>
            <div className="phase6DataHealthGrid" aria-label="数据检查">
              <HealthItem label={credentialReady ? "登录凭证可用" : "登录凭证待登录"} ok={credentialReady} />
              <HealthItem label={status?.health.readable ? "本地数据库可用" : "本地数据库待检查"} ok={Boolean(status?.health.readable)} />
              <HealthItem label={presetMatches > 0 ? "本地赛季有数据" : "本地赛季等待数据"} ok={presetMatches > 0} />
              <HealthItem label="没有待处理失败" ok={!warnings.length} />
            </div>
          </aside>
        </div>

        <section className="commandCenterTools" aria-labelledby="command-center-tools-title">
          <div><span className="phase6Eyebrow">应用工具</span><h2 id="command-center-tools-title">本地应用与支持</h2></div>
          <div className="phase6SettingsLauncherGrid syncSecondaryActions">
                <SettingsPlaceholder icon={<BadgeInfo size={17} />} label="版本号" onClick={() => setNotice("版本号入口后续提供")} />
                <SettingsPlaceholder icon={<FileText size={17} />} label="日志文件" onClick={() => setNotice("日志文件入口后续提供")} />
                <SettingsPlaceholder icon={<FolderOpen size={17} />} label="数据存放路径" onClick={() => setNotice("数据存放路径入口后续提供")} />
                <SettingsPlaceholder icon={<Github size={17} />} label="Github地址" onClick={() => setNotice("Github地址入口后续提供")} />
          </div>
        </section>
        </> : (
          <section className="commandCenterAccountEmpty" id="local-data" tabIndex={-1} aria-labelledby="command-center-empty-title">
            <h2 id="command-center-empty-title">尚未添加账号</h2>
            <p>添加账号后即可同步和查看本地数据。</p>
          </section>
        )}

        {accountStatus ? <div className="accountManagerPanel">
          {(busyAccountAction?.kind === "add" || busyAccountAction?.kind === "relogin") && (
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
            busyAction={busyAccountAction}
            onAdd={runAddAccount}
            onSwitch={runSwitchAccount}
            onRelogin={runReloginAccount}
            onRemove={runRemoveAccount}
          />
        </div> : null}
      </QaRegion>
    </section>
  );
}


function ProfileInlineStat({ label, value }: { label: string; value: string | number }) {
  return (
    <span>
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  );
}

function SettingsPlaceholder({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button className="ghostButton phase6SettingsPlaceholderAction" type="button" onClick={onClick}>
      {icon}
      {label}
    </button>
  );
}

function HealthItem({ label, ok }: { label: string; ok: boolean }) {
  return (
    <span className={ok ? "good" : "warn"}>
      <CheckCircle2 size={14} />
      {label}
    </span>
  );
}
