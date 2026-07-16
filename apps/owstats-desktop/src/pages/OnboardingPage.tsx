import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  BadgeInfo,
  CheckCircle2,
  Download,
  FileText,
  FolderOpen,
  Github,
  LogIn,
  LogOut,

  UserRound,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  cancelDashenLogin,
  checkAppUpdate,
  confirmLoginTargetSwitch,
  getOnboardingStatus,
  getPlayerProfile,
  getLoginTargetStatus,
  clearPlayerTarget,
  keepCurrentPlayerTarget,
  logout,
  retryLoginTargetBinding,
  startDashenLogin,
  syncSeason,
} from "../shared/api";
import { DataFact, formatTimestamp } from "../features/command-center/SeasonHealthPanel";
import { resolveCommandCenterSeason, SeasonSyncWorkbench } from "../features/command-center/SeasonSyncWorkbench";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import { clearMatchesPageCache } from "../shared/matchesPageCache";
import { PageHeader } from "../shared/page-shell";
import type { CredentialLoginProgress, LoginTargetStatus, OnboardingStatus, PlayerProfile, SyncProgress } from "../shared/types";

type DashboardSnapshot = {
  status: OnboardingStatus;
  profile: PlayerProfile;
  targetStatus: LoginTargetStatus;
};

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
  const [profile, setProfile] = useState<PlayerProfile | null>(null);
  const [targetStatus, setTargetStatus] = useState<LoginTargetStatus | null>(null);
  const [selectedSeason, setSelectedSeason] = useState<number | null>(null);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncNotice, setSyncNotice] = useState<string | null>(null);
  const [loginProgress, setLoginProgress] = useState<CredentialLoginProgress | null>(null);
  const [syncingSeason, setSyncingSeason] = useState<number | null>(null);
  const [loggingIn, setLoggingIn] = useState(false);
  const [cancellingLogin, setCancellingLogin] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
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
    const cached = dashboardCache;
    if (cached) {
      applyDashboardSnapshot(cached);
    }
    void loadDashboard();
  }, []);

  async function loadDashboard() {
    const requestGeneration = ++dashboardRequestGenerationRef.current;
    if (mountedRef.current) {
      setError(null);
    }
    try {
      const [nextStatus, nextProfile, nextTargetStatus] = await Promise.all([
        getOnboardingStatus(),
        getPlayerProfile(),
        getLoginTargetStatus(),
      ]);
      if (!isCurrentDashboardRequest(requestGeneration, dashboardRequestGenerationRef.current, mountedRef.current)) {
        return;
      }
      const snapshot = { status: nextStatus, profile: nextProfile, targetStatus: nextTargetStatus };
      dashboardCache = snapshot;
      applyDashboardSnapshot(snapshot);
    } catch (err) {
      if (isCurrentDashboardRequest(requestGeneration, dashboardRequestGenerationRef.current, mountedRef.current)) {
        setError(productErrorMessage(err));
      }
    }
  }

  function applyDashboardSnapshot(snapshot: DashboardSnapshot) {
    setStatus(snapshot.status);
    setProfile(snapshot.profile);
    setTargetStatus(snapshot.targetStatus);
    setSelectedSeason((current) => resolveCommandCenterSeason(
      current ?? snapshot.status.currentSeason,
      snapshot.status.syncableSeasons,
      snapshot.status.currentSeason,
    ));
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

  async function runLogin() {
    if (!beginAccountOperation()) {
      return;
    }
    setLoggingIn(true);
    setNotice(null);
    setError(null);
    try {
      const result = await startDashenLogin();
      if (mountedRef.current) {
        setStatus((current) => (current ? { ...current, credentialStatus: result.status } : current));
        setTargetStatus(result.target);
        setNotice(result.target.message);
      }
      await loadDashboard();
    } catch (err) {
      const loginError = `登录或识别失败：${productErrorMessage(err)}。请重新识别。`;
      await loadDashboard();
      if (mountedRef.current) {
        setError(loginError);
      }
    } finally {
      if (mountedRef.current) {
        setLoggingIn(false);
      }
      releaseOperationLock();
    }
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

  async function runSeasonSync(season: number) {
    if (!acquireOperationLock()) {
      setSyncError("当前正在处理账号操作，请完成后再同步。");
      return;
    }
    dashboardRequestGenerationRef.current += 1;
    setSyncingSeason(season);
    setNotice(null);
    setError(null);
    setSyncNotice(null);
    setSyncError(null);
    setSyncProgress(null);
    try {
      const summary = await syncSeason(season);
      clearMatchesPageCache();
      await loadDashboard();
      setSyncNotice(`S${season} 赛季同步完成：本次导入 ${summary.importedMatches} 场，对局详情 ${summary.importedDetails} 场。`);
    } catch (err) {
      setSyncError(productErrorMessage(err));
    } finally {
      setSyncingSeason(null);
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

  async function runLogout() {
    if (!beginAccountOperation()) {
      return;
    }
    setLoggingOut(true);
    setNotice(null);
    setError(null);
    try {
      const result = await logout();
      if (mountedRef.current) {
        setNotice(result.message);
        setLoginProgress(null);
        await loadDashboard();
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(`退出登录失败：${productErrorMessage(err)}。请重试。`);
      }
    } finally {
      if (mountedRef.current) {
        setLoggingOut(false);
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
  const accountOperationBusy = loggingIn || loggingOut || targetAction !== null;
  const hasCurrentTarget = Boolean(targetStatus?.currentBnet);
  const primaryTargetState = targetStatus && ["switchRequired", "bindFailed", "unidentified", "verificationRequired", "mismatchKept"].includes(targetStatus.state)
    ? targetStatus.state
    : null;
  const hasTargetPrimaryAction = primaryTargetState !== null;
  const showDetectedLogin = Boolean(
    targetStatus?.currentBnet &&
    targetStatus.detectedBnet &&
    normalizeLoginTargetName(targetStatus.currentBnet) !== normalizeLoginTargetName(targetStatus.detectedBnet),
  );
  const warnings = status?.health.warnings ?? [];
  const displayName = profile?.playerId ?? "未登录玩家";
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

      <div className="commandCenterMetricsRow" aria-label="指挥台状态摘要">
        <DataFact label="登录状态" value={credentialReady ? "已登录" : "待登录"} tone={credentialReady ? "good" : "warn"} />
        <DataFact label="本地对局" value={`${presetMatches} 场`} tone={presetMatches > 0 ? "good" : "warn"} />
        <DataFact label="覆盖率" value={`${coveragePercent}%`} tone={coveragePercent >= 80 ? "good" : "warn"} />
        <DataFact label="最近同步" value={lastSync} tone={lastSync === "-" ? "warn" : "neutral"} />
      </div>

      <QaRegion id="CMD-PROFILE" className="phase6QaBlockRegion">
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
                    <button ref={primaryTargetActionRef} className="primaryButton" type="button" onClick={runLogin} disabled={accountOperationBusy || syncingSeason !== null}>
                      {loggingIn ? "识别中…" : "重新识别"}
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
                  progress={syncProgress}
                  syncError={syncError}
                  syncNotice={syncNotice}
                  onSelectSeason={selectSyncSeason}
                  onSync={runSeasonSync}
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
            <div className="commandCenterAccountActions">
                {!hasTargetPrimaryAction && (
                  <button className="primaryButton" type="button" onClick={runLogin} disabled={accountOperationBusy || syncingSeason !== null}>
                    <LogIn size={18} />
                    {loggingIn ? "登录识别中…" : credentialReady ? "重新登录" : "登录网易大神"}
                  </button>
                )}
                {loggingIn && (
                  <button className="ghostButton" type="button" onClick={cancelLogin} disabled={cancellingLogin}>
                    <X size={18} />
                    {cancellingLogin ? "取消中…" : "取消"}
                  </button>
                )}
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

        <section className="commandCenterDangerZone" aria-label="账号危险操作">
          <div><strong>退出当前账号</strong><p>退出后会清除本地登录凭证，但不会删除已经导入的赛季数据。</p></div>
          <button className="dangerButton" type="button" onClick={runLogout} disabled={!credentialReady || accountOperationBusy || syncingSeason !== null}>
            <LogOut size={18} />
            {loggingOut ? "退出中…" : "退出登录"}
          </button>
        </section>
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
