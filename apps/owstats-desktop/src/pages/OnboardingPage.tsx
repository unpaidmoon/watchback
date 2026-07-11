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
  RefreshCw,
  UserRound,
  X,
} from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  cancelDashenLogin,
  checkAppUpdate,
  getOnboardingStatus,
  getPersonalSeasonReview,
  getPlayerProfile,
  logout,
  startDashenLogin,
  syncSeason,
} from "../shared/api";
import { achievementBadgeUrl } from "../shared/assets";
import { GlassPanel, HeroAvatar } from "../shared/phase6Components";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { activeBadgeLevel, activeTopAchievements, romanLevels } from "../shared/reviewAchievements";
import type { ReviewAchievement } from "../shared/reviewAchievements";
import { hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import type { CredentialLoginProgress, OnboardingStatus, PersonalSeasonReview, PlayerProfile, SeasonDataStatus, SyncProgress } from "../shared/types";

type DashboardSnapshot = {
  status: OnboardingStatus;
  profile: PlayerProfile;
  review: PersonalSeasonReview | null;
};

const PROFILE_REVIEW_SEASON = 22;
const FIXED_COMMAND_CENTER_SEASONS = [
  { season: 21, label: "S21" },
  { season: 22, label: "S22" },
  { season: 23, label: "S23" },
] as const;
const dashboardCache = new Map<number, DashboardSnapshot>();

export function OnboardingPage() {
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [profile, setProfile] = useState<PlayerProfile | null>(null);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);
  const [loginProgress, setLoginProgress] = useState<CredentialLoginProgress | null>(null);
  const [syncingSeason, setSyncingSeason] = useState<number | null>(null);
  const [loggingIn, setLoggingIn] = useState(false);
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
      const [nextStatus, nextProfile, nextReview] = await Promise.all([
        getOnboardingStatus(),
        getPlayerProfile(),
        getPersonalSeasonReview(PROFILE_REVIEW_SEASON).catch(() => null),
      ]);
      const snapshot = { status: nextStatus, profile: nextProfile, review: nextReview };
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
  }

  async function runLogin() {
    setLoggingIn(true);
    setNotice(null);
    setError(null);
    try {
      const result = await startDashenLogin();
      setStatus((current) => (current ? { ...current, credentialStatus: result.status } : current));
      await loadDashboard();
    } catch (err) {
      setError(productErrorMessage(err));
      await loadDashboard();
    } finally {
      setLoggingIn(false);
    }
  }

  async function cancelLogin() {
    setError(null);
    try {
      await cancelDashenLogin();
      setLoginProgress({ phase: "cancelled", message: "已取消登录等待。" });
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setLoggingIn(false);
    }
  }

  async function runSeasonSync(season: number) {
    setSyncingSeason(season);
    setNotice(null);
    setError(null);
    setSyncProgress(null);
    try {
      const summary = await syncSeason(season);
      await loadDashboard();
      setNotice(`S${season} 赛季同步完成：本次导入 ${summary.importedMatches} 场，对局详情 ${summary.importedDetails} 场。`);
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setSyncingSeason(null);
    }
  }

  async function runLogout() {
    setNotice(null);
    setError(null);
    try {
      const result = await logout();
      setNotice(result.message);
      setLoginProgress(null);
      dashboardCache.delete(PROFILE_REVIEW_SEASON);
      await loadDashboard();
    } catch (err) {
      setError(productErrorMessage(err));
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
  const warnings = status?.health.warnings ?? [];
  const displayName = profile?.playerId ?? (status?.credentialStatus.roleId ? `角色 ${status.credentialStatus.roleId}` : "未登录玩家");
  const lastSync = formatTimestamp(status?.localData.lastSyncedOrImportedAt);
  const canUseData = Boolean(status?.canEnterReview);
  const presetMatches = status?.localData.presetMatches ?? 0;
  const detailMatches = status?.localData.detailMatches ?? 0;
  const coveragePercent = presetMatches > 0 ? Math.min(100, Math.round((detailMatches / presetMatches) * 100)) : 0;
  const seasonStatuses = FIXED_COMMAND_CENTER_SEASONS.map((fixedSeason) => {
    return (
      status?.seasonStatuses.find((item) => item.season === fixedSeason.season) ?? {
        season: fixedSeason.season,
        matchCount: 0,
        detailCount: 0,
        normalMatchCount: 0,
        arenaMatchCount: 0,
        failedDetailCount: 0,
        lastSyncedOrImportedAt: null,
        coveragePercent: 0,
      }
    );
  });
  const commonHeroes = review?.heroPool.mainHeroes.slice(0, 3) ?? [];
  const topAchievements = review ? activeTopAchievements(review, 3) : [];

  return (
    <section className="workspace onboardingWorkspace phase6CommandCenter">
      {(error || notice) && (
        <div className={`errorBanner inline phase6RecoverableError ${notice && !error ? "good" : ""}`}>
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
              <div className="phase8TargetPlayerPanel" aria-label="当前同步账号">
                <div className="phase8TargetPlayerSummary">
                  <span className="phase6Eyebrow">当前同步账号</span>
                  <strong>{credentialReady ? "当前登录账号" : "尚未识别当前账号"}</strong>
                  <p>{credentialReady ? "仅同步当前浏览器登录账号的赛季数据" : status?.credentialStatus.message ?? "请登录网易大神"}</p>
                </div>
              </div>
              <div className="phase8SeasonStatusGrid" aria-label="赛季本地数据状态">
                {seasonStatuses.map((seasonStatus) => (
                  <SeasonStatusCard
                    key={seasonStatus.season}
                    seasonStatus={seasonStatus}
                    credentialReady={credentialReady}
                    syncingSeason={syncingSeason}
                    progress={syncProgress?.season === seasonStatus.season ? syncProgress : null}
                    onSync={() => runSeasonSync(seasonStatus.season)}
                  />
                ))}
              </div>

              <div className="phase6SettingsLauncherGrid">
                <button className="primaryButton" type="button" onClick={runLogin} disabled={loggingIn}>
                  <LogIn size={18} />
                  {credentialReady ? "重新登录" : "登录网易大神"}
                </button>
                {loggingIn && (
                  <button className="ghostButton" type="button" onClick={cancelLogin}>
                    <X size={18} />
                    取消
                  </button>
                )}
                <button className="ghostButton" type="button" onClick={runUpdateCheck} disabled={checkingUpdate}>
                  <Download size={18} />
                  {checkingUpdate ? "检查中" : "检查更新"}
                </button>
                <button className="dangerButton" type="button" onClick={runLogout} disabled={!credentialReady}>
                  <LogOut size={18} />
                  退出登录
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

function SeasonStatusCard({
  seasonStatus,
  credentialReady,
  syncingSeason,
  progress,
  onSync,
}: {
  seasonStatus: SeasonDataStatus;
  credentialReady: boolean;
  syncingSeason: number | null;
  progress: SyncProgress | null;
  onSync: () => void;
}) {
  const isSyncing = syncingSeason === seasonStatus.season;
  const isSyncBlocked = syncingSeason !== null && !isSyncing;
  const hasData = seasonStatus.matchCount > 0;
  const statusLabel = !credentialReady
    ? "需要登录"
    : isSyncing
        ? "同步中"
        : hasData
          ? "本地数据可用"
          : "无本地数据";

  return (
    <article className="phase8SeasonStatusCard">
      <div className="phase8SeasonStatusHeader">
        <div>
          <span className="phase6Eyebrow">固定赛季</span>
          <h3>{`S${seasonStatus.season}`}</h3>
        </div>
        <span className={credentialReady ? "good" : "warn"}>{statusLabel}</span>
      </div>
      <div className="phase8SeasonStatusStats">
        <DataFact label="本地对局" value={`${seasonStatus.matchCount} 场`} tone={hasData ? "good" : "warn"} />
        <DataFact label="详情数" value={`${seasonStatus.detailCount} 场`} tone={seasonStatus.detailCount > 0 ? "good" : "warn"} />
        <DataFact label="普通对局" value={`${seasonStatus.normalMatchCount} 场`} tone="neutral" />
        <DataFact label="角斗对局" value={`${seasonStatus.arenaMatchCount} 场`} tone="neutral" />
        <DataFact label="待补详情" value={`${seasonStatus.failedDetailCount} 条`} tone={seasonStatus.failedDetailCount > 0 ? "warn" : "good"} />
        <DataFact label="最近同步/导入" value={formatTimestamp(seasonStatus.lastSyncedOrImportedAt)} tone={seasonStatus.lastSyncedOrImportedAt ? "neutral" : "warn"} />
      </div>
      <div className="phase8SeasonCoverage">
        <div>
          <span>详情覆盖率</span>
          <strong>{seasonStatus.coveragePercent}%</strong>
        </div>
        <div className="phase6CoverageTrack" aria-hidden="true">
          <span style={{ width: `${seasonStatus.coveragePercent}%` }} />
        </div>
      </div>
      <div className="phase8SeasonSyncFooter">
        <span>
          {isSyncing
            ? readableSyncProgress(progress)
            : !credentialReady
              ? "登录后可同步"
              : "当前登录账号可用"}
        </span>
        <button className="primaryButton" type="button" onClick={onSync} disabled={!credentialReady || isSyncBlocked || isSyncing}>
          <RefreshCw size={18} />
          {isSyncing ? "同步中" : `同步 S${seasonStatus.season}`}
        </button>
      </div>
    </article>
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

function DataFact({ label, value, tone }: { label: string; value: string; tone: "good" | "warn" | "neutral" }) {
  return (
    <span className={tone}>
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
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

function formatTimestamp(value: string | null | undefined) {
  if (!value) {
    return "暂无同步";
  }

  const compact = value.replace(/(\.\d{3})\d+/, "$1");
  const parsed = new Date(compact);
  if (Number.isNaN(parsed.getTime())) {
    return value.replace("T", " ").replace(/(\.\d+)?([+-]\d\d:?\d\d|Z)?$/, "");
  }

  const pad = (part: number) => String(part).padStart(2, "0");
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())} ${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`;
}
