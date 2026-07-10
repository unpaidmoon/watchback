import {
  AlertCircle,
  BarChart3,
  ExternalLink,
  Sparkles,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { SeasonAnalysisModules } from "./FriendAnalyticsPage";
import { getPersonalSeasonReview, getSeasonAnalysis } from "../shared/api";
import { achievementBadgeUrl, rankAssetUrl } from "../shared/assets";
import { GlassPanel, HeroAvatar, MapPreview, StatusBadge } from "../shared/phase6Components";
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

export function PersonalReviewPage() {
  const [params, setParams] = useSearchParams();
  const requestedSeason = parseSeasonParam(params.get("season"));
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>(requestedSeason);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [analysis, setAnalysis] = useState<SeasonAnalysis | null>(null);
  const [analysisLoading, setAnalysisLoading] = useState(false);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedAchievement, setSelectedAchievement] = useState<ReviewAchievement | null>(null);

  useEffect(() => {
    setSelectedSeason(requestedSeason);
  }, [requestedSeason]);

  useEffect(() => {
    setError(null);
    void getPersonalSeasonReview(selectedSeason)
      .then((nextReview) => {
        setReview(nextReview);
        if (selectedSeason === undefined && nextReview.summary.season) {
          setSelectedSeason(nextReview.summary.season);
        }
      })
      .catch((err) => setError(productErrorMessage(err)));
  }, [selectedSeason]);

  useEffect(() => {
    if (selectedSeason === undefined) return;
    setAnalysisLoading(true);
    setAnalysisError(null);
    void getSeasonAnalysis(selectedSeason)
      .then(setAnalysis)
      .catch((err) => setAnalysisError(productErrorMessage(err)))
      .finally(() => setAnalysisLoading(false));
  }, [selectedSeason]);

  function changeSeason(nextSeason: number) {
    setSelectedSeason(nextSeason);
    setReview(null);
    setAnalysis(null);
    const nextParams = new URLSearchParams(params);
    nextParams.set("season", String(nextSeason));
    setParams(nextParams);
  }

  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }

  if (!review) return <section className="workspace">正在加载个人赛季战报...</section>;

  const achievements = buildSharedReviewAchievements(review);
  const analytics = analysis?.friend ?? null;
  const titleSeason = selectedSeason ?? review.summary.season;

  return (
    <section className="workspace reviewWorkspace phase6ReviewWorkspace">
      <GlassPanel className="phase6RoleReport" tone="solid">
        <div className="phase6RoleReportHeader">
          <div>
            <span className="phase6Eyebrow">战报</span>
            <h2>{titleSeason ? `S${titleSeason} 赛季表现报告` : "赛季表现报告"}</h2>
            <p>按职责汇总当前段位、胜率、样本和 KDA，下面保留可追溯的复盘证据。</p>
          </div>
          <div className="phase6CommandStatus">
            <label className="seasonSelect phase8SeasonSelect">
              <span>赛季</span>
              <select value={selectedSeason ?? ""} onChange={(event) => changeSeason(Number(event.target.value))}>
                {selectableSeasons.map((season) => (
                  <option key={season} value={season}>
                    S{season}
                  </option>
                ))}
              </select>
            </label>
            <StatusBadge tone="good">
              <BarChart3 size={15} /> {review.summary.totalMatches} 场
            </StatusBadge>
            <StatusBadge>
              <Sparkles size={15} /> {review.phase5Highlights.sGradeCount} 场 S 级
            </StatusBadge>
          </div>
        </div>

        <div className="phase6RoleRankGrid">
          {review.roleSummaries.map((role) => (
            <RoleRankCard key={role.roleType} role={role} />
          ))}
        </div>
      </GlassPanel>

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

      {selectedAchievement ? (
        <AchievementDialog achievement={selectedAchievement} onClose={() => setSelectedAchievement(null)} />
      ) : null}

      <SeasonAnalysisModules analysis={analysis} analytics={analytics} loading={analysisLoading} error={analysisError} />
    </section>
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
          <button className="phase6IconButton" type="button" onClick={onClose} aria-label="关闭成就详情">
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
