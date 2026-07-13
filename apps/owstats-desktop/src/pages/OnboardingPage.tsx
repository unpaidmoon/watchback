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
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  cancelDashenLogin,
  checkAppUpdate,
  confirmLoginTargetSwitch,
  getOnboardingStatus,
  getPersonalSeasonReview,
  getPlayerProfile,
  getLoginTargetStatus,
  clearPlayerTarget,
  keepCurrentPlayerTarget,
  logout,
  retryLoginTargetBinding,
  startDashenLogin,
  syncSeason,
} from "../shared/api";
import { achievementBadgeUrl } from "../shared/assets";
import { GlassPanel, HeroAvatar } from "../shared/phase6Components";
import { DataFact, formatTimestamp, SeasonStatusCard } from "../features/command-center/SeasonHealthPanel";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { activeBadgeLevel, activeTopAchievements, romanLevels } from "../shared/reviewAchievements";
import type { ReviewAchievement } from "../shared/reviewAchievements";
import { hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import { clearMatchesPageCache } from "../shared/matchesPageCache";
import type { CredentialLoginProgress, LoginTargetStatus, OnboardingStatus, PersonalSeasonReview, PlayerProfile, SyncProgress } from "../shared/types";

type DashboardSnapshot = {
  status: OnboardingStatus;
  profile: PlayerProfile;
  review: PersonalSeasonReview | null;
  targetStatus: LoginTargetStatus;
};

type TargetAction = "confirm" | "keep" | "retry" | "clear";

const PROFILE_REVIEW_SEASON = 22;
const dashboardCache = new Map<number, DashboardSnapshot>();

export function OnboardingPage() {
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [profile, setProfile] = useState<PlayerProfile | null>(null);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [targetStatus, setTargetStatus] = useState<LoginTargetStatus | null>(null);
  const [seasonInput, setSeasonInput] = useState("23");
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);
  const [loginProgress, setLoginProgress] = useState<CredentialLoginProgress | null>(null);
  const [syncingSeason, setSyncingSeason] = useState<number | null>(null);
  const [loggingIn, setLoggingIn] = useState(false);
  const [cancellingLogin, setCancellingLogin] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
  const [targetAction, setTargetAction] = useState<TargetAction | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hasDesktopBackend()) {
      return;
    }

    let unlistenSync: (() => void) | undefined;
    let unlistenLogin: (() => void) | undefined;
    void listen<SyncProgress>("sync-progress", (event) => setSyncProgress(event.payload)).then((handler) => {
      unlistenSync = handler;
    });
    void listen<CredentialLoginProgress>("credential-login-progress", (event) => setLoginProgress(event.payload)).then((handler) => {
      unlistenLogin = handler;
    });
    return () => {
      unlistenSync?.();
      unlistenLogin?.();
    };
  }, []);

  useEffect(() => {
    const cached = dashboardCache.get(PROFILE_REVIEW_SEASON);
    if (cached) {
      applyDashboardSnapshot(cached);
    }
    void loadDashboard();
  }, []);

  async function loadDashboard() {
    setError(null);
    try {
      const [nextStatus, nextProfile, nextReview, nextTargetStatus] = await Promise.all([
        getOnboardingStatus(),
        getPlayerProfile(),
        getPersonalSeasonReview(PROFILE_REVIEW_SEASON).catch(() => null),
        getLoginTargetStatus(),
      ]);
      const snapshot = { status: nextStatus, profile: nextProfile, review: nextReview, targetStatus: nextTargetStatus };
      dashboardCache.set(PROFILE_REVIEW_SEASON, snapshot);
      applyDashboardSnapshot(snapshot);
    } catch (err) {
      setError(productErrorMessage(err));
    }
  }

  function applyDashboardSnapshot(snapshot: DashboardSnapshot) {
    setStatus(snapshot.status);
    setProfile(snapshot.profile);
    setReview(snapshot.review);
    setTargetStatus(snapshot.targetStatus);
  }

  async function runLogin() {
    setLoggingIn(true);
    setNotice(null);
    setError(null);
    try {
      const result = await startDashenLogin();
      setStatus((current) => (current ? { ...current, credentialStatus: result.status } : current));
      setTargetStatus(result.target);
      setNotice(result.target.message);
      await loadDashboard();
    } catch (err) {
      setError(`登录或识别失败：${productErrorMessage(err)}。请重新识别。`);
      await loadDashboard();
    } finally {
      setLoggingIn(false);
    }
  }

  async function cancelLogin() {
    setCancellingLogin(true);
    setError(null);
    try {
      await cancelDashenLogin();
      setLoginProgress({ phase: "cancelled", message: "已取消登录等待。" });
    } catch (err) {
      setError(`取消登录失败：${productErrorMessage(err)}。请重试。`);
    } finally {
      setLoggingIn(false);
      setCancellingLogin(false);
    }
  }

  async function runSeasonSync(season: number) {
    setSyncingSeason(season);
    setNotice(null);
    setError(null);
    setSyncProgress(null);
    try {
      const summary = await syncSeason(season);
      clearMatchesPageCache();
      await loadDashboard();
      setNotice(`S${season} 赛季同步完成：本次导入 ${summary.importedMatches} 场，对局详情 ${summary.importedDetails} 场。`);
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setSyncingSeason(null);
    }
  }

  async function runConfirmTargetSwitch() {
    setTargetAction("confirm");
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await confirmLoginTargetSwitch();
      setTargetStatus(nextTarget);
      setNotice(nextTarget.message);
    } catch (err) {
      setError(`切换目标失败：${productErrorMessage(err)}。请重试或重新识别。`);
    } finally {
      setTargetAction(null);
    }
  }

  async function runKeepCurrentTarget() {
    setTargetAction("keep");
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await keepCurrentPlayerTarget();
      setTargetStatus(nextTarget);
      setNotice(nextTarget.message);
    } catch (err) {
      setError(`保留原目标失败：${productErrorMessage(err)}。请重试或重新识别。`);
    } finally {
      setTargetAction(null);
    }
  }

  async function runRetryTargetBinding() {
    setTargetAction("retry");
    setError(null);
    setNotice(null);
    try {
      const nextTarget = await retryLoginTargetBinding();
      setTargetStatus(nextTarget);
      setNotice(nextTarget.message);
    } catch (err) {
      setError(`重新绑定失败：${productErrorMessage(err)}。请重试或重新识别。`);
    } finally {
      setTargetAction(null);
    }
  }

  async function runClearTarget() {
    setTargetAction("clear");
    setNotice(null);
    setError(null);
    try {
      const result = await clearPlayerTarget();
      setTargetStatus(result.target);
      setNotice("已清除当前目标，请重新识别登录账号。");
    } catch (err) {
      setError(`清除目标失败：${productErrorMessage(err)}。请重试。`);
    } finally {
      setTargetAction(null);
    }
  }

  async function runLogout() {
    setLoggingOut(true);
    setNotice(null);
    setError(null);
    try {
      const result = await logout();
      setNotice(result.message);
      setLoginProgress(null);
      dashboardCache.delete(PROFILE_REVIEW_SEASON);
      await loadDashboard();
    } catch (err) {
      setError(`退出登录失败：${productErrorMessage(err)}。请重试。`);
    } finally {
      setLoggingOut(false);
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
  const syncTargetReady = credentialReady && Boolean(targetStatus?.syncAllowed);
  const hasCurrentTarget = Boolean(targetStatus?.currentBnet);
  const warnings = status?.health.warnings ?? [];
  const displayName = profile?.playerId ?? (status?.credentialStatus.roleId ? `角色 ${status.credentialStatus.roleId}` : "未登录玩家");
  const lastSync = formatTimestamp(status?.localData.lastSyncedOrImportedAt);
  const canUseData = Boolean(status?.canEnterReview);
  const presetMatches = status?.localData.presetMatches ?? 0;
  const detailMatches = status?.localData.detailMatches ?? 0;
  const coveragePercent = presetMatches > 0 ? Math.min(100, Math.round((detailMatches / presetMatches) * 100)) : 0;
  const selectedSeason = Number.parseInt(seasonInput, 10);
  const seasonStatuses = Number.isSafeInteger(selectedSeason) && selectedSeason > 0
    ? [status?.seasonStatuses.find((item) => item.season === selectedSeason) ?? {
        season: selectedSeason,
        matchCount: 0,
        detailCount: 0,
        normalMatchCount: 0,
        arenaMatchCount: 0,
        failedDetailCount: 0,
        lastSyncedOrImportedAt: null,
        coveragePercent: 0,
      }]
    : [];
  const commonHeroes = review?.heroPool.mainHeroes.slice(0, 3) ?? [];
  const topAchievements = review ? activeTopAchievements(review, 3) : [];

  return (
    <section className="workspace onboardingWorkspace phase6CommandCenter">
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

      <QaRegion id="CMD-PROFILE" className="phase6QaBlockRegion">
        <GlassPanel className="phase6PlayerPass" tone="solid">
          <div className="phase6PlayerPassRail">
            <div className="phase6ProfileAvatar">
              {profile?.avatarUrl ? <img src={profile.avatarUrl} alt="" /> : <UserRound size={42} />}
            </div>
          </div>
          <div className="phase6PassIdentity">
            <h2>{displayName}</h2>
            <p>{profile?.title ?? "暂无头衔"}</p>
            <div className="phase6ProfileInlineStats" aria-label="账号摘要">
              <ProfileInlineStat label="赞赏等级" value={profile?.endorsementLevel ?? "-"} />
              <ProfileInlineStat label="游戏时间" value={profile?.playTimeHours != null ? `${profile.playTimeHours.toFixed(1)} 小时` : "-"} />
            </div>
          </div>
          <div className="phase6ProfileShowcase" aria-label="常用英雄与已激活成就">
            <section className="phase6ProfileShowcaseGroup" aria-label="常用英雄">
              <h3>常用英雄</h3>
              <div className="phase6ProfileHeroStrip">
                {Array.from({ length: 3 }).map((_, index) => {
                  const hero = commonHeroes[index];
                  return (
                    <div className="phase6ProfileShowcaseItem" key={hero ? `${hero.hero}-${index}` : `empty-hero-${index}`}>
                      {hero ? <HeroAvatar heroKey={hero.hero} name={hero.hero} size="sm" /> : <span className="phase6ProfileHeroEmpty" aria-hidden="true" />}
                      <span className="phase6ProfileHeroName">{hero?.hero ?? "--"}</span>
                    </div>
                  );
                })}
              </div>
            </section>
            <section className="phase6ProfileShowcaseGroup phase6ProfileAchievementGroup phase6ProfileAchievementInset" aria-label="成就展示">
              <h3>成就展示</h3>
              <div className="phase6ProfileAchievementBadges">
                {Array.from({ length: 3 }).map((_, index) => {
                  const achievement = topAchievements[index];
                  return (
                    <div className="phase6ProfileShowcaseItem" key={achievement?.key ?? `empty-achievement-${index}`}>
                      {achievement ? <ProfileAchievementBadge achievement={achievement} /> : <span className="phase6ProfileAchievementEmpty" aria-hidden="true" />}
                      <span className="phase6ProfileAchievementName">{achievement?.title ?? "--"}</span>
                    </div>
                  );
                })}
              </div>
            </section>
          </div>

          <div className="phase6UnifiedCommandPanel">
            <section className="phase6CoverageMeter phase6DataCoverageCard" aria-label="数据覆盖度">
              <div className="phase6CoveragePrimary">
                <div>
                  <span className="phase6Eyebrow">数据覆盖度</span>
                  <strong>{coveragePercent}%</strong>
                  <p>{presetMatches > 0 ? `${detailMatches}/${presetMatches} 场详情已就绪` : "暂无对局数据"}</p>
                </div>
                <div className="phase6DataFacts" aria-label="数据摘要">
                  <DataFact label="登录凭证" value={credentialReady ? "可用" : "待登录"} tone={credentialReady ? "good" : "warn"} />
                  <DataFact label="本地对局" value={`${presetMatches} 场`} tone={presetMatches > 0 ? "good" : "warn"} />
                  <DataFact label="最近同步" value={lastSync} tone={lastSync === "-" ? "warn" : "neutral"} />
                </div>
              </div>
              <div className="phase6CoverageTrack" aria-hidden="true">
                <span style={{ width: `${coveragePercent}%` }} />
              </div>
              <div className="phase6CoverageFooter">
                <span>{canUseData ? "数据已就绪" : "等待数据"}</span>
                <strong>{readableSyncProgress(syncProgress)}</strong>
              </div>
              <div className="phase6DataHealthGrid" aria-label="数据检查">
                <HealthItem label={credentialReady ? "登录凭证可用" : "登录凭证待登录"} ok={credentialReady} />
                <HealthItem label={status?.health.readable ? "本地数据库可用" : "本地数据库待检查"} ok={Boolean(status?.health.readable)} />
                <HealthItem label={presetMatches > 0 ? "本地赛季有数据" : "本地赛季等待数据"} ok={presetMatches > 0} />
                <HealthItem label={presetMatches > 0 && detailMatches >= presetMatches ? "对局详情完整" : "对局详情待补全"} ok={presetMatches > 0 && detailMatches >= presetMatches} />
                <HealthItem label="没有待处理失败" ok={!warnings.length} />
              </div>
            </section>

            <section className="phase6UnifiedActions" aria-label="设置选项">
              <section className="syncTargetStrip" aria-label="当前目标玩家">
                <div className="phase8TargetPlayerSummary">
                  <span className="phase6Eyebrow">当前目标玩家</span>
                  <strong>{targetStatus?.currentBnet ?? targetStatus?.detectedBnet ?? "尚未识别账号"}</strong>
                  {targetStatus?.state === "switchRequired" && (
                    <div className="syncTargetComparison" aria-label="账号切换确认">
                      <span>原目标 <strong>{targetStatus.currentBnet ?? "未知"}</strong></span>
                      <span>本次账号 <strong>{targetStatus.detectedBnet ?? "未识别"}</strong></span>
                    </div>
                  )}
                  <p>{targetStatus?.message ?? status?.credentialStatus.message ?? "请登录网易大神并自动识别当前账号"}</p>
                </div>
                <div className="syncTargetDecision">
                  {targetStatus?.state === "ready" && <span className="good">当前目标已确认，可同步</span>}
                  {targetStatus?.state === "switchRequired" && (
                    <>
                      <button className="primaryButton" type="button" onClick={runConfirmTargetSwitch} disabled={targetAction !== null || loggingIn || !targetStatus.canConfirmSwitch}>
                        {targetAction === "confirm" ? "切换中…" : "切换到此账号"}
                      </button>
                      <button className="ghostButton" type="button" onClick={runKeepCurrentTarget} disabled={targetAction !== null || loggingIn}>
                        {targetAction === "keep" ? "保留中…" : "保留原目标"}
                      </button>
                    </>
                  )}
                  {targetStatus?.state === "bindFailed" && targetStatus.canRetry ? (
                    <button className="primaryButton" type="button" onClick={runRetryTargetBinding} disabled={targetAction !== null || loggingIn}>
                      {targetAction === "retry" ? "重新绑定中…" : "重新绑定"}
                    </button>
                  ) : targetStatus && ["unidentified", "verificationRequired", "mismatchKept", "bindFailed"].includes(targetStatus.state) ? (
                    <button className="primaryButton" type="button" onClick={runLogin} disabled={loggingIn || targetAction !== null}>
                      {loggingIn ? "识别中…" : "重新识别"}
                    </button>
                  ) : null}
                  <button className="ghostButton" type="button" onClick={runClearTarget} disabled={!hasCurrentTarget || targetAction !== null || loggingIn}>
                    {targetAction === "clear" ? "清除中…" : "清除"}
                  </button>
                </div>
              </section>
              <section className="syncSeasonWorkbench" aria-label="赛季同步">
                <div className="syncSeasonPicker">
                  <span className="phase6Eyebrow">赛季同步</span>
                  <div>
                    <button className="ghostButton" type="button" onClick={() => setSeasonInput(String(Math.max(1, selectedSeason - 1)))} aria-label="上一赛季">−</button>
                    <label> S <input aria-label="赛季号" inputMode="numeric" value={seasonInput} onChange={(event) => setSeasonInput(event.target.value)} /> </label>
                    <button className="ghostButton" type="button" onClick={() => setSeasonInput(String(selectedSeason + 1))} aria-label="下一赛季">+</button>
                  </div>
                  <p>已有赛季：{status?.availableSeasons.length ? status.availableSeasons.map((season) => `S${season}`).join(" · ") : "暂无"}</p>
                </div>
                <div className="phase8SeasonStatusGrid" aria-label="赛季本地数据状态">
                  {seasonStatuses.map((seasonStatus) => (
                    <SeasonStatusCard
                      key={seasonStatus.season}
                      seasonStatus={seasonStatus}
                      credentialReady={syncTargetReady}
                      syncingSeason={syncingSeason}
                      progress={syncProgress?.season === seasonStatus.season ? syncProgress : null}
                      onSync={() => runSeasonSync(seasonStatus.season)}
                    />
                  ))}
                </div>
              </section>

              <div className="phase6SettingsLauncherGrid syncSecondaryActions">
                <button className="primaryButton" type="button" onClick={runLogin} disabled={loggingIn || cancellingLogin || targetAction !== null}>
                  <LogIn size={18} />
                  {loggingIn ? "登录识别中…" : credentialReady ? "重新登录" : "登录网易大神"}
                </button>
                {loggingIn && (
                  <button className="ghostButton" type="button" onClick={cancelLogin} disabled={cancellingLogin}>
                    <X size={18} />
                    {cancellingLogin ? "取消中…" : "取消"}
                  </button>
                )}
                <button className="ghostButton" type="button" onClick={runUpdateCheck} disabled={checkingUpdate}>
                  <Download size={18} />
                  {checkingUpdate ? "检查中" : "检查更新"}
                </button>
                <button className="dangerButton" type="button" onClick={runLogout} disabled={!credentialReady || loggingOut}>
                  <LogOut size={18} />
                  {loggingOut ? "退出中…" : "退出登录"}
                </button>
                <SettingsPlaceholder icon={<BadgeInfo size={17} />} label="版本号" onClick={() => setNotice("版本号入口后续提供")} />
                <SettingsPlaceholder icon={<FileText size={17} />} label="日志文件" onClick={() => setNotice("日志文件入口后续提供")} />
                <SettingsPlaceholder icon={<FolderOpen size={17} />} label="数据存放路径" onClick={() => setNotice("数据存放路径入口后续提供")} />
                <SettingsPlaceholder icon={<Github size={17} />} label="Github地址" onClick={() => setNotice("Github地址入口后续提供")} />
              </div>
            </section>
          </div>
        </GlassPanel>
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

function ProfileAchievementBadge({ achievement }: { achievement: ReviewAchievement }) {
  const activeLevel = activeBadgeLevel(achievement);

  return (
    <img
      className="phase6ProfileAchievementToken"
      src={achievementBadgeUrl(achievement.key, activeLevel)}
      alt={`${achievement.title} ${romanLevels[activeLevel - 1]}`}
      title={`${achievement.title} ${romanLevels[activeLevel - 1]}`}
    />
  );
}

function SettingsPlaceholder({ icon, label, onClick }: { icon: ReactNode; label: string; onClick: () => void }) {
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
