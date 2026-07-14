import { ExternalLink, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { FriendAnalysisModules, MapAnalysisModules } from "./FriendAnalyticsPage";
import {
  buildReviewHref,
  parseReviewView,
  reviewScrollKey,
  scheduleReviewScrollRestore,
  type ReviewView,
} from "../features/review/reviewNavigation";
import { getPersonalSeasonReview, getSeasonAnalysis } from "../shared/api";
import { achievementBadgeUrl, rankAssetUrl } from "../shared/assets";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
import { buildReviewAchievements as buildSharedReviewAchievements } from "../shared/reviewAchievements";
import type { EvidenceLink, PersonalSeasonReview, RoleRankSummary, SeasonAnalysis } from "../shared/types";

type ReviewAchievement = {
  key: AchievementKey;
  title: string;
  metric: string;
  detail: string;
  value: number;
  thresholds: readonly [number, number, number];
  unit: string;
  currentLevel: 0 | 1 | 2 | 3;
  progress: number;
  evidence?: EvidenceLink;
};

type AchievementKey = "win-streak" | "win-rate" | "s-grade" | "carry" | "signature" | "peak" | "hero-focus" | "switching";

const romanLevels = ["I", "II", "III"] as const;
const selectableSeasons = [23, 22, 21] as const;
const reviewViewLabels: Record<ReviewView, string> = {
  overview: "概览",
  roles: "职责与英雄",
  friends: "队友协作",
  maps: "地图",
  achievements: "成就",
};

export function createLatestRequestGuard() {
  let latestRequest = 0;

  return {
    begin() {
      latestRequest += 1;
      return latestRequest;
    },
    isCurrent(request: number) {
      return latestRequest === request;
    },
    invalidate(request?: number) {
      if (request === undefined || latestRequest === request) latestRequest += 1;
    },
  };
}

export function hasReviewSeasonData(
  review: Pick<PersonalSeasonReview, "summary">,
): review is Pick<PersonalSeasonReview, "summary"> & { summary: PersonalSeasonReview["summary"] & { season: number } } {
  return typeof review.summary.season === "number" && review.summary.season > 0;
}

export function shouldRequestReviewSeason(selectedSeason: number | undefined, loadedSeason: number | null) {
  return selectedSeason === undefined || selectedSeason !== loadedSeason;
}

const inFlightReviewRequests = new Map<string, Promise<PersonalSeasonReview>>();

export function requestPersonalSeasonReviewOnce(
  season: number | undefined,
  load: (requestedSeason?: number) => Promise<PersonalSeasonReview> = getPersonalSeasonReview,
) {
  const requestKey = season === undefined ? "latest" : `season:${season}`;
  const existingRequest = inFlightReviewRequests.get(requestKey);
  if (existingRequest) return existingRequest;

  const pendingRequest = load(season);
  const request = pendingRequest.finally(() => {
    if (inFlightReviewRequests.get(requestKey) === request) {
      inFlightReviewRequests.delete(requestKey);
    }
  });
  inFlightReviewRequests.set(requestKey, request);
  return request;
}

export function ReviewEmptyState({ onAction = () => undefined }: { onAction?: () => void }) {
  return (
    <PageEmptyState
      title="暂无可用赛季战报"
      description="当前本地数据库没有可用于复盘的赛季数据。"
      actionLabel="前往指挥台"
      onAction={onAction}
    />
  );
}

export function PersonalReviewPage() {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const requestedSeason = parseSeasonParam(params.get("season"));
  const view = parseReviewView(params.get("view"));
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>(requestedSeason);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [analysis, setAnalysis] = useState<SeasonAnalysis | null>(null);
  const [analysisLoading, setAnalysisLoading] = useState(false);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedAchievement, setSelectedAchievement] = useState<ReviewAchievement | null>(null);
  const reviewRequestGuard = useRef(createLatestRequestGuard());
  const analysisRequestGuard = useRef(createLatestRequestGuard());
  const loadedReviewSeasonRef = useRef<number | null>(null);

  useEffect(() => {
    if (params.get("view") === view) return;
    const nextParams = new URLSearchParams(params);
    nextParams.set("view", view);
    setParams(nextParams, { replace: true });
  }, [params, setParams, view]);

  useEffect(() => {
    if (requestedSeason === selectedSeason) return;
    reviewRequestGuard.current.invalidate();
    analysisRequestGuard.current.invalidate();
    loadedReviewSeasonRef.current = null;
    setSelectedSeason(requestedSeason);
    setReview(null);
    setAnalysis(null);
    setError(null);
    setAnalysisError(null);
    setAnalysisLoading(false);
  }, [requestedSeason]);

  function requestReview() {
    if (!shouldRequestReviewSeason(selectedSeason, loadedReviewSeasonRef.current)) return;
    const request = reviewRequestGuard.current.begin();
    setReview(null);
    setError(null);
    void requestPersonalSeasonReviewOnce(selectedSeason)
      .then((nextReview) => {
        if (!reviewRequestGuard.current.isCurrent(request)) return;
        setReview(nextReview);
        const backendSeason = nextReview.summary.season;
        loadedReviewSeasonRef.current = typeof backendSeason === "number" && backendSeason > 0 ? backendSeason : null;
        if (selectedSeason === undefined && typeof backendSeason === "number" && backendSeason > 0) {
          setSelectedSeason(backendSeason);
          const nextParams = new URLSearchParams(params);
          nextParams.set("season", String(backendSeason));
          nextParams.set("view", view);
          setParams(nextParams, { replace: true });
        }
      })
      .catch((err) => {
        if (reviewRequestGuard.current.isCurrent(request)) setError(productErrorMessage(err));
      });

    return () => reviewRequestGuard.current.invalidate(request);
  }

  useEffect(() => requestReview(), [selectedSeason]);

  useEffect(() => {
    if (selectedSeason === undefined) {
      setAnalysis(null);
      setAnalysisError(null);
      setAnalysisLoading(false);
      return;
    }
    const request = analysisRequestGuard.current.begin();
    setAnalysis(null);
    setAnalysisLoading(true);
    setAnalysisError(null);
    void getSeasonAnalysis(selectedSeason)
      .then((nextAnalysis) => {
        if (analysisRequestGuard.current.isCurrent(request)) setAnalysis(nextAnalysis);
      })
      .catch((err) => {
        if (analysisRequestGuard.current.isCurrent(request)) setAnalysisError(productErrorMessage(err));
      })
      .finally(() => {
        if (analysisRequestGuard.current.isCurrent(request)) setAnalysisLoading(false);
      });

    return () => analysisRequestGuard.current.invalidate(request);
  }, [selectedSeason]);

  function changeSeason(nextSeason: number) {
    setSelectedSeason(nextSeason);
    setReview(null);
    setAnalysis(null);
    const nextParams = new URLSearchParams(params);
    nextParams.set("season", String(nextSeason));
    setParams(nextParams);
  }

  function changeView(nextView: ReviewView) {
    const nextParams = new URLSearchParams(params);
    nextParams.set("view", nextView);
    setSelectedAchievement(null);
    setParams(nextParams);
  }

  function renderHeader(titleSeason: number | null | undefined) {
    return (
      <PageHeader
        title="战报"
        description={titleSeason ? `S${titleSeason} 赛季表现与协作复盘` : "查看赛季表现与协作复盘"}
        actions={
          <PageSeasonSelect
            value={selectedSeason}
            seasons={selectableSeasons}
            onChange={(season) => {
              if (season !== undefined) changeSeason(season);
            }}
          />
        }
      />
    );
  }

  if (error) {
    return (
      <section className="workspace reviewWorkspace phase6ReviewWorkspace">
        {renderHeader(selectedSeason)}
        <PageErrorState message={error} onRetry={() => void requestReview()} />
      </section>
    );
  }

  if (!review) {
    return (
      <section className="workspace reviewWorkspace phase6ReviewWorkspace">
        {renderHeader(selectedSeason)}
        <PageLoadingState message="正在加载个人赛季战报..." />
      </section>
    );
  }

  if (!hasReviewSeasonData(review)) {
    return (
      <section className="workspace reviewWorkspace phase6ReviewWorkspace">
        {renderHeader(selectedSeason)}
        <ReviewEmptyState onAction={() => navigate("/onboarding")} />
      </section>
    );
  }

  const achievements = buildSharedReviewAchievements(review);
  const analytics = analysis?.friend ?? null;
  const titleSeason = selectedSeason ?? review.summary.season;

  return (
    <section className="workspace reviewWorkspace phase6ReviewWorkspace">
      {renderHeader(titleSeason)}
      <ReviewScrollRestorer season={titleSeason} view={view} />

      <nav className="reviewTabs" aria-label="战报内容">
        <ReviewTabButton value="overview" view={view} onSelect={changeView} />
        <ReviewTabButton value="roles" view={view} onSelect={changeView} />
        <ReviewTabButton value="friends" view={view} onSelect={changeView} />
        <ReviewTabButton value="maps" view={view} onSelect={changeView} />
        <ReviewTabButton value="achievements" view={view} onSelect={changeView} />
      </nav>

      <div className="reviewViewGrid" data-view={view}>
        {view === "overview" && <ReviewOverview review={review} />}
        {view === "roles" && <RoleAndHeroView review={review} />}
        {view === "friends" && (
          <FriendAnalysisModules
            analytics={analytics}
            loading={analysisLoading}
            error={analysisError}
            season={titleSeason}
            sourceView={view}
          />
        )}
        {view === "maps" && <MapAnalysisModules analysis={analysis} loading={analysisLoading} error={analysisError} season={titleSeason} />}
        {view === "achievements" && (
          <GlassPanel className="phase6AchievementPanel" tone="solid">
            <header className="phase6AchievementHeader">
              <div>
                <h3>成就墙</h3>
              </div>
            </header>
            <div className="phase6AchievementWall">
              {achievements.map((achievement) => (
                <AchievementCard key={achievement.key} achievement={achievement} onOpen={() => setSelectedAchievement(achievement)} />
              ))}
            </div>
          </GlassPanel>
        )}
      </div>

      {selectedAchievement ? <AchievementDialog achievement={selectedAchievement} onClose={() => setSelectedAchievement(null)} /> : null}
    </section>
  );
}

function ReviewScrollRestorer({ season, view }: { season: number; view: ReviewView }) {
  useLayoutEffect(() => {
    const href = buildReviewHref(season, view);
    const key = reviewScrollKey(href);
    const stored = sessionStorage.getItem(key);
    if (stored === null) return;
    const scrollY = Number(stored);
    if (!Number.isFinite(scrollY)) {
      sessionStorage.removeItem(key);
      return;
    }
    return scheduleReviewScrollRestore(scrollY, {
      requestFrame: (callback) => requestAnimationFrame(callback),
      cancelFrame: (frame) => cancelAnimationFrame(frame),
      scrollTo: (top) => window.scrollTo({ top }),
      readScrollY: () => window.scrollY,
      readMaxScrollY: () => Math.max(document.documentElement.scrollHeight, document.body.scrollHeight) - window.innerHeight,
      removeStored: () => sessionStorage.removeItem(key),
    });
  }, [season, view]);

  return null;
}

function ReviewTabButton({
  value,
  view,
  onSelect,
}: {
  value: ReviewView;
  view: ReviewView;
  onSelect: (view: ReviewView) => void;
}) {
  const active = value === view;
  return (
    <button
      type="button"
      value={value}
      aria-current={active ? "page" : undefined}
      className={active ? "active" : ""}
      onClick={() => onSelect(value)}
    >
      {reviewViewLabels[value]}
    </button>
  );
}

function ReviewOverview({ review }: { review: PersonalSeasonReview }) {
  return (
    <>
      <GlassPanel className="reviewOverviewPanel reviewOverviewPrimary" tone="solid">
        <header>
          <span className="phase6Eyebrow">赛季概览</span>
          <h2>稳定表现</h2>
          <p>以完整赛季样本汇总胜负、胜率与高光表现。</p>
        </header>
        <div className="reviewOverviewMetrics">
          <Metric label="对局" value={`${review.summary.totalMatches} 场`} />
          <Metric label="胜负" value={`${review.summary.wins} 胜 / ${review.summary.losses} 负`} />
          <Metric label="胜率" value={formatWinRate(review.summary.winRate, 0)} />
          <Metric label="S 级" value={`${review.phase5Highlights.sGradeCount} 场`} />
        </div>
      </GlassPanel>
      <GlassPanel className="reviewOverviewPanel reviewOverviewSecondary" tone="solid">
        <header>
          <span className="phase6Eyebrow">本季摘要</span>
          <h2>关键表现</h2>
        </header>
        <div className="reviewOverviewMetrics compact">
          <Metric label="最长连胜" value={`${review.streaks.longestWinStreak} 局`} />
          <Metric label="带飞局" value={`${review.phase5Highlights.carryCount} 场`} />
          <Metric label="主职责" value={review.roleSummaries[0]?.roleLabel ?? "样本不足"} />
        </div>
      </GlassPanel>
    </>
  );
}

function RoleAndHeroView({ review }: { review: PersonalSeasonReview }) {
  const mainHero = review.heroPool.mainHeroes[0];

  return (
    <GlassPanel className="phase6RoleReport reviewRoleHeroPanel" tone="solid">
      <div className="phase6RoleReportHeader">
        <div>
          <span className="phase6Eyebrow">职责与英雄</span>
          <h2>职责段位与英雄池</h2>
          <p>保留三职责摘要，并用稳定指标说明英雄选择范围。</p>
        </div>
      </div>
      <div className="phase6RoleRankGrid">
        {review.roleSummaries.map((role) => (
          <RoleRankCard key={role.roleType} role={role} />
        ))}
      </div>
      <div className="reviewHeroSummary">
        <Metric label="英雄池" value={`${review.heroPool.totalHeroes} 名`} />
        <Metric label="主要英雄" value={mainHero?.hero ?? "样本不足"} />
        <Metric label="第一英雄占比" value={formatWinRate(review.heroPool.top1Share, 0)} />
        <Metric label="场均英雄数" value={review.heroPool.avgHeroesPerMatch.toFixed(1)} />
      </div>
    </GlassPanel>
  );
}

function parseSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return Number.isFinite(season) ? season : undefined;
}

function RoleRankCard({ role }: { role: RoleRankSummary }) {
  const tierLabel = role.rankSubTier > 0 ? String(role.rankSubTier) : "";

  return (
    <article className="phase6RoleRankCard">
      <div className="phase6RoleRankName">{role.roleLabel}</div>
      <div className="phase6RoleRankIconWrap">
        <img className="phase6RoleRankIcon" src={rankAssetUrl(role.rankKey)} alt={`${role.roleLabel} ${role.rankKey}`} />
        {tierLabel ? <span className="phase6RoleRankTier">{tierLabel}</span> : null}
      </div>
      <div className="phase6RoleRankStats">
        <Metric label="胜率" value={formatWinRate(role.winRate, 0)} />
        <Metric label="场次" value={role.matches} />
        <Metric label="KDA" value={role.matches > 0 ? role.kda.toFixed(2) : "-"} />
      </div>
    </article>
  );
}

function buildReviewAchievements(review: PersonalSeasonReview): ReviewAchievement[] {
  const peakBest = review.personalBests[0];
  const mainHero = review.heroPool.mainHeroes[0];
  const winRatePercent = review.summary.winRate * 100;
  const mainHeroSharePercent = review.heroPool.top1Share * 100;
  const signatureCount = review.phase5Highlights.mostCommonHighlight ? Math.max(1, review.phase5Highlights.sGradeCount) : 0;
  const peakValue = peakBest?.value ?? 0;

  return [
    buildAchievement({
      key: "win-streak",
      title: "常胜将军",
      metric: `最长连胜 ${review.streaks.longestWinStreak} 局`,
      detail: "连续表现奖章",
      value: review.streaks.longestWinStreak,
      thresholds: [3, 5, 8],
      unit: "局",
    }),
    buildAchievement({
      key: "win-rate",
      title: "胜率机器",
      metric: `${formatWinRate(review.summary.winRate, 0)} 胜率`,
      detail: `${review.summary.wins} 胜 / ${review.summary.losses} 负`,
      value: winRatePercent,
      thresholds: [50, 55, 60],
      unit: "%",
    }),
    buildAchievement({
      key: "s-grade",
      title: "高光制造者",
      metric: `S 级表现 ${review.phase5Highlights.sGradeCount} 场`,
      detail: `${review.summary.totalMatches} 场样本中的峰值表现`,
      value: review.phase5Highlights.sGradeCount,
      thresholds: [5, 15, 30],
      unit: "场 S",
    }),
    buildAchievement({
      key: "carry",
      title: "战局核心",
      metric: `带飞局 ${review.phase5Highlights.carryCount} 场`,
      detail: "高价值正向表现",
      value: review.phase5Highlights.carryCount,
      thresholds: [5, 12, 25],
      unit: "场",
    }),
    buildAchievement({
      key: "signature",
      title: "招牌打法",
      metric: review.phase5Highlights.mostCommonHighlight ?? "暂无高频高光",
      detail: "出现最多的个人高光标签",
      value: signatureCount,
      thresholds: [3, 8, 15],
      unit: "次",
    }),
    buildAchievement({
      key: "peak",
      title: "巅峰一战",
      metric: peakBest ? `${peakBest.label} ${peakBest.value}` : "暂无单场峰值",
      detail: peakBest ? `${peakBest.resultLabel} / ${peakBest.heroName} / ${peakBest.mapName}` : "需要更多对局样本",
      value: peakValue,
      thresholds: buildPeakThresholds(peakValue),
      unit: "",
      evidence: peakBest?.evidence,
    }),
    buildAchievement({
      key: "hero-focus",
      title: "本命专精",
      metric: `第一英雄占比 ${formatWinRate(review.heroPool.top1Share, 0)}`,
      detail: mainHero ? `${mainHero.hero} ${mainHero.matches} 场` : "暂无主力英雄",
      value: mainHeroSharePercent,
      thresholds: [25, 40, 55],
      unit: "%",
      evidence: mainHero?.evidence,
    }),
    buildAchievement({
      key: "switching",
      title: "战术变阵",
      metric: `使用 ${review.heroPool.totalHeroes} 名英雄`,
      detail: `切换率 ${formatWinRate(review.heroPool.switchMatchRate, 0)}`,
      value: review.heroPool.totalHeroes,
      thresholds: [5, 10, 15],
      unit: "名英雄",
    }),
  ];
}

function buildAchievement(input: Omit<ReviewAchievement, "currentLevel" | "progress">): ReviewAchievement {
  return {
    ...input,
    currentLevel: achievementLevel(input.value, input.thresholds),
    progress: achievementProgress(input.value, input.thresholds),
  };
}

function achievementLevel(value: number, thresholds: readonly [number, number, number]): 0 | 1 | 2 | 3 {
  if (value >= thresholds[2]) return 3;
  if (value >= thresholds[1]) return 2;
  if (value >= thresholds[0]) return 1;
  return 0;
}

function achievementProgress(value: number, thresholds: readonly [number, number, number]) {
  if (value >= thresholds[2]) return 100;
  const nextThreshold = thresholds.find((threshold) => value < threshold) ?? thresholds[2];
  return Math.max(0, Math.min(100, (value / nextThreshold) * 100));
}

function buildPeakThresholds(value: number): [number, number, number] {
  const base = Math.max(1, value);
  return [1, Math.max(2, Math.ceil(base * 0.8)), Math.max(3, Math.ceil(base * 1.05))];
}

function activeBadgeLevel(achievement: ReviewAchievement): 1 | 2 | 3 {
  if (achievement.currentLevel === 2) return 2;
  if (achievement.currentLevel === 3) return 3;
  return 1;
}

function formatThreshold(value: number, unit: string) {
  if (unit === "%") return `${value}%`;
  if (!unit) return `${value}`;
  return `${value} ${unit}`;
}

function AchievementCard({ achievement, onOpen }: { achievement: ReviewAchievement; onOpen: () => void }) {
  const activeLevel = activeBadgeLevel(achievement);
  const nextLevel = achievement.currentLevel === 0 ? "I" : achievement.currentLevel === 1 ? "II" : achievement.currentLevel === 2 ? "III" : "III";
  const nextThreshold = achievement.thresholds[Math.min(achievement.currentLevel, 2)];

  return (
    <article className="phase6AchievementCard">
      <button className="phase6AchievementCardButton" type="button" onClick={onOpen}>
        <img
          className={`phase6AchievementMedalImage ${achievement.currentLevel === 0 ? "locked" : ""}`}
          src={achievementBadgeUrl(achievement.key, activeLevel)}
          alt={`${achievement.title} ${romanLevels[activeLevel - 1]}`}
        />
        <div className="phase6AchievementCopy">
          <strong>
            {achievement.title}
            <span>{achievement.currentLevel > 0 ? romanLevels[achievement.currentLevel - 1] : "未激活"}</span>
          </strong>
          <span>{achievement.metric}</span>
          <small>{achievement.detail}</small>
        </div>
        <div className="phase6AchievementProgress" aria-label={`成就进度 ${Math.round(achievement.progress)}%`}>
          <span style={{ width: `${achievement.progress}%` }} />
        </div>
        <div className="phase6AchievementTierRail" aria-hidden="true">
          {romanLevels.map((level, index) => {
            const tier = (index + 1) as 1 | 2 | 3;
            const unlocked = achievement.currentLevel >= tier;
            const active = achievement.currentLevel === tier;
            return (
              <span key={level} className={`${unlocked ? "unlocked" : "locked"} ${active ? "active" : ""}`}>
                {level}
              </span>
            );
          })}
        </div>
        <small className="phase6AchievementNext">
          {achievement.currentLevel >= 3 ? "已达成最高等级" : `下一等级 ${nextLevel}：${formatThreshold(nextThreshold, achievement.unit)}`}
        </small>
      </button>
      {achievement.evidence ? <Evidence evidence={achievement.evidence} /> : null}
    </article>
  );
}

function AchievementDialog({ achievement, onClose }: { achievement: ReviewAchievement; onClose: () => void }) {
  const nextThreshold = achievement.thresholds[Math.min(achievement.currentLevel, 2)];
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButtonRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "Tab") {
        event.preventDefault();
        closeButtonRef.current?.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [onClose]);

  return (
    <div className="phase6AchievementModalBackdrop" role="presentation" onClick={onClose}>
      <section
        className="phase6AchievementModal"
        role="dialog"
        aria-modal="true"
        aria-label={`${achievement.title} 详情`}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="phase6AchievementModalHeader">
          <div>
            <span className="phase6Eyebrow">成就详情</span>
            <h3>{achievement.title}</h3>
            <p>{achievement.detail}</p>
          </div>
          <button ref={closeButtonRef} className="phase6IconButton" type="button" onClick={onClose} aria-label="关闭成就详情">
            <X size={18} />
          </button>
        </header>

        <div className="phase6AchievementBadgeTrack">
          {romanLevels.map((level, index) => {
            const tier = (index + 1) as 1 | 2 | 3;
            const unlocked = achievement.currentLevel >= tier;
            const active = achievement.currentLevel === tier || (achievement.currentLevel === 0 && tier === 1);
            return (
              <div key={level} className={`phase6AchievementBadgeStage ${unlocked ? "unlocked" : "locked"} ${active ? "active" : ""}`}>
                <img src={achievementBadgeUrl(achievement.key, tier)} alt={`${achievement.title} ${level}`} />
                <strong>{level}</strong>
                <span>{formatThreshold(achievement.thresholds[index], achievement.unit)}</span>
              </div>
            );
          })}
        </div>

        <div className="phase6AchievementModalStats">
          <Metric label="当前" value={achievement.metric} />
          <Metric label="当前等级" value={achievement.currentLevel > 0 ? romanLevels[achievement.currentLevel - 1] : "未激活"} />
          <Metric label={achievement.currentLevel >= 3 ? "最高等级" : "下一等级"} value={achievement.currentLevel >= 3 ? "III" : formatThreshold(nextThreshold, achievement.unit)} />
        </div>

        <div className="phase6AchievementModalProgress">
          <span style={{ width: `${achievement.progress}%` }} />
        </div>
      </section>
    </div>
  );
}

function Evidence({ evidence }: { evidence: EvidenceLink }) {
  return (
    <Link className="evidenceLink phase6EvidenceLink" to={evidence.route}>
      {evidence.map && <MapPreview mapKey={evidence.map} name={evidence.map} className="phase6EvidenceMap" />}
      {evidence.hero && <HeroAvatar heroKey={evidence.hero} name={evidence.hero} size="sm" />}
      <span>{evidence.label}</span>
      <ExternalLink size={14} />
    </Link>
  );
}

function Metric({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
