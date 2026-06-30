import { AlertCircle, BarChart3, ExternalLink, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { getPersonalSeasonReview } from "../shared/api";
import { GlassPanel, HeroAvatar, MapPreview, MetricCard, StatusBadge } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
import type { EvidenceLink, PersonalSeasonReview } from "../shared/types";

export function PersonalReviewPage() {
  const [params] = useSearchParams();
  const season = Number(params.get("season") || 22);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setError(null);
    void getPersonalSeasonReview(season)
      .then(setReview)
      .catch((err) => setError(productErrorMessage(err)));
  }, [season]);

  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }

  if (!review) return <section className="workspace">正在加载个人赛季复盘...</section>;

  return (
    <section className="workspace reviewWorkspace phase6ReviewWorkspace">
      <header className="phase6PageHeader">
        <div>
          <span className="phase6Eyebrow">个人复盘</span>
          <h2>S{review.summary.season ?? season} 赛季报告</h2>
          <p>把赛季表现、关键结论和证据对局集中成一份可行动报告。</p>
        </div>
        <div className="phase6CommandStatus">
          <StatusBadge tone="good">
            <BarChart3 size={15} /> {review.summary.totalMatches} 场
          </StatusBadge>
          <StatusBadge>
            <Sparkles size={15} /> {review.phase5Highlights.sGradeCount} 场 S 级
          </StatusBadge>
        </div>
      </header>

      <section className="phase6MatchMetrics">
        <MetricCard label="预设 / 开放" value={String(review.summary.totalMatches)} meta="赛季总样本" />
        <MetricCard label="胜率" value={formatWinRate(review.summary.winRate, 0)} meta={`${review.summary.wins} 胜 / ${review.summary.losses} 负`} />
        <MetricCard label="带飞局" value={String(review.phase5Highlights.carryCount)} meta="高价值正向表现" />
        <MetricCard label="最长连胜" value={String(review.phase5Highlights.longestWinStreak)} meta={review.phase5Highlights.mostCommonHighlight ?? "暂无高频高光"} />
      </section>

      <GlassPanel className="phase6ReviewPanel" tone="solid">
        <header>
          <span className="phase6Eyebrow">核心结论</span>
          <h3>赛季重点发现</h3>
        </header>
        <div className="conclusionList phase6ConclusionList">
          {review.topConclusions.map((item) => (
            <article key={`${item.kind}-${item.title}`}>
              <strong>{item.title}</strong>
              <p>{item.body}</p>
              <Evidence evidence={item.evidence} />
            </article>
          ))}
        </div>
      </GlassPanel>

      <div className="reviewGrid phase6ReviewGrid">
        <GlassPanel className="phase6ReviewPanel" tone="solid">
          <header>
            <span className="phase6Eyebrow">赛季基准</span>
            <h3>稳定表现底线</h3>
          </header>
          <div className="explainMetrics">
            <Metric label="场次" value={review.baseline.matchCount} />
            <Metric label="击杀" value={review.baseline.avgKills.toFixed(1)} />
            <Metric label="死亡" value={review.baseline.avgDeaths.toFixed(1)} />
            <Metric label="伤害" value={Math.round(review.baseline.avgDamage)} />
          </div>
          <p className="explainText">短局阈值：{review.baseline.shortMatchThresholdSec} 秒。基准用于复盘判断，不影响对局列表保留。</p>
        </GlassPanel>

        <GlassPanel className="phase6ReviewPanel" tone="solid">
          <header>
            <span className="phase6Eyebrow">个人最佳</span>
            <h3>有证据的高光场次</h3>
          </header>
          <div className="bestList phase6BestList">
            {review.personalBests.map((best) => (
              <article key={best.metric}>
                <HeroAvatar heroKey={best.heroName} name={best.heroName} size="sm" />
                <div>
                  <strong>
                    {best.label}: {best.value}
                  </strong>
                  <span>
                    {best.resultLabel} / {best.heroName} / {best.mapName}
                  </span>
                  <Evidence evidence={best.evidence} />
                </div>
              </article>
            ))}
          </div>
        </GlassPanel>
      </div>

      <div className="reviewGrid phase6ReviewGrid">
        <GlassPanel className="phase6ReviewPanel" tone="solid">
          <header>
            <span className="phase6Eyebrow">英雄池</span>
            <h3>{review.heroPool.label}</h3>
          </header>
          <div className="summaryGrid">
            <Metric label="英雄数" value={review.heroPool.totalHeroes} />
            <Metric label="第一英雄占比" value={formatWinRate(review.heroPool.top1Share, 0)} />
            <Metric label="前三英雄占比" value={formatWinRate(review.heroPool.top3Share, 0)} />
            <Metric label="切换率" value={formatWinRate(review.heroPool.switchMatchRate, 0)} />
          </div>
          <div className="breakdownList topGap">
            {review.heroPool.mainHeroes.map((hero) => (
              <div key={hero.hero}>
                <span className="phase6HeroCell">
                  <HeroAvatar heroKey={hero.hero} name={hero.hero} size="sm" />
                  {hero.hero}
                </span>
                <strong>
                  {hero.matches} 场 / {formatWinRate(hero.winRate, 0)}
                </strong>
              </div>
            ))}
          </div>
        </GlassPanel>

        <GlassPanel className="phase6ReviewPanel" tone="solid">
          <header>
            <span className="phase6Eyebrow">连续表现</span>
            <h3>胜负节奏</h3>
          </header>
          <div className="summaryGrid">
            <Metric label="最长连胜" value={review.streaks.longestWinStreak} />
            <Metric label="最长连败" value={review.streaks.longestLossStreak} />
          </div>
          <p className="explainText">连败只作为复盘背景，不进入顶部核心摘要。</p>
        </GlassPanel>
      </div>
    </section>
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
