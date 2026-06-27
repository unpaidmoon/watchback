import { BarChart3, ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { getPersonalSeasonReview } from "../shared/api";
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
      .catch((err) => setError(String(err)));
  }, [season]);

  if (error) return <div className="errorBanner">{error}</div>;
  if (!review) return <section className="workspace">正在加载个人赛季复盘...</section>;

  return (
    <section className="workspace reviewWorkspace">
      <header className="sectionHeader">
        <div>
          <span className="eyebrow">个人复盘</span>
          <h2>S{review.summary.season ?? season} 个人赛季复盘</h2>
        </div>
        <BarChart3 size={22} />
      </header>

      <div className="summaryGrid">
        <Metric label="预设/开放" value={review.summary.totalMatches} />
        <Metric label="胜率" value={`${Math.round(review.summary.winRate * 100)}%`} />
        <Metric label="基准场次" value={review.summary.baselineMatches} />
        <Metric label="基准排除" value={review.baseline.excludedTotal} />
      </div>

      <section className="playerSection topGap">
        <h3>核心结论</h3>
        <div className="conclusionList">
          {review.topConclusions.map((item) => (
            <article key={`${item.kind}-${item.title}`}>
              <strong>{item.title}</strong>
              <p>{item.body}</p>
              <Evidence evidence={item.evidence} />
            </article>
          ))}
        </div>
      </section>

      <div className="reviewGrid">
        <section className="playerSection">
          <h3>赛季基准</h3>
          <div className="explainMetrics">
            <Metric label="场次" value={review.baseline.matchCount} />
            <Metric label="击杀" value={review.baseline.avgKills} />
            <Metric label="死亡" value={review.baseline.avgDeaths} />
            <Metric label="伤害" value={Math.round(review.baseline.avgDamage)} />
          </div>
          <p className="explainText">短局阈值：{review.baseline.shortMatchThresholdSec} 秒。排除只用于基准，对局仍保留在列表和总览。</p>
          <div className="breakdownList">
            {review.baseline.exclusionReasons.map((item) => (
              <div key={item.reason}>
                <span>{item.label}</span>
                <strong>{item.count}</strong>
              </div>
            ))}
          </div>
        </section>

        <section className="playerSection">
          <h3>个人最佳</h3>
          <div className="bestList">
            {review.personalBests.map((best) => (
              <article key={best.metric}>
                <strong>
                  {best.label}: {best.value}
                </strong>
                <span>
                  {best.resultLabel} / {best.heroName} / {best.mapName}
                </span>
                <Evidence evidence={best.evidence} />
              </article>
            ))}
          </div>
        </section>
      </div>

      <div className="reviewGrid">
        <section className="playerSection">
          <h3>英雄池稳定性</h3>
          <p className="explainText">{review.heroPool.label}</p>
          <div className="summaryGrid">
            <Metric label="英雄数" value={review.heroPool.totalHeroes} />
            <Metric label="第一英雄占比" value={`${Math.round(review.heroPool.top1Share * 100)}%`} />
            <Metric label="前三英雄占比" value={`${Math.round(review.heroPool.top3Share * 100)}%`} />
            <Metric label="切换率" value={`${Math.round(review.heroPool.switchMatchRate * 100)}%`} />
          </div>
          <div className="breakdownList topGap">
            {review.heroPool.mainHeroes.map((hero) => (
              <div key={hero.hero}>
                <span>{hero.hero}</span>
                <strong>
                  {hero.matches} 场 / {Math.round(hero.winRate * 100)}%
                </strong>
              </div>
            ))}
          </div>
        </section>

        <section className="playerSection">
          <h3>连续胜负</h3>
          <div className="summaryGrid">
            <Metric label="最长连胜" value={review.streaks.longestWinStreak} />
            <Metric label="最长连败" value={review.streaks.longestLossStreak} />
          </div>
          <p className="explainText">最长连败仅在详情模块展示，不进入顶部核心摘要。</p>
        </section>
      </div>
    </section>
  );
}

function Evidence({ evidence }: { evidence: EvidenceLink }) {
  return (
    <Link className="evidenceLink" to={evidence.route}>
      <ExternalLink size={14} />
      {evidence.label}
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
