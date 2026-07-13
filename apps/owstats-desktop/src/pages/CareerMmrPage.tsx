import { AlertCircle, BarChart3, Clock3, ShieldCheck, TrendingUp } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  getCareerMmrBuildStatus,
  getCareerMmrOverview,
  getRoleMmrHistory,
  getRoleMmrModeBreakdown,
  getRoleMmrSeasonBreakdown,
  rebuildCareerMmr,
} from "../shared/api";
import {
  careerModeLabels,
  careerRoleLabels,
  buildCareerMmrChart,
  createCareerMmrRequestGuard,
  formatCareerDate,
  formatCareerMmr,
  formatCareerPercent,
  signedCareerDelta,
} from "../shared/careerMmrFormat";
import { productErrorMessage } from "../shared/phase6Shell";
import type {
  CareerMmrBuildStatus,
  CareerMmrHistory,
  CareerMmrModeBreakdown,
  CareerMmrOverview,
  CareerMmrSeasonBreakdown,
  CareerRole,
} from "../shared/types";

const roles: CareerRole[] = ["tank", "damage", "support"];
const roleNamesForAccessibility = "重装、输出、支援";

export function CareerMmrPage() {
  const [overview, setOverview] = useState<CareerMmrOverview | null>(null);
  const [selectedRole, setSelectedRole] = useState<CareerRole>("tank");
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>();
  const [history, setHistory] = useState<CareerMmrHistory | null>(null);
  const [modes, setModes] = useState<CareerMmrModeBreakdown | null>(null);
  const [seasons, setSeasons] = useState<CareerMmrSeasonBreakdown | null>(null);
  const [buildStatus, setBuildStatus] = useState<CareerMmrBuildStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const detailRequestGuard = useRef(createCareerMmrRequestGuard());

  useEffect(() => {
    let active = true;
    setLoading(true);
    Promise.all([getCareerMmrOverview(), getCareerMmrBuildStatus()])
      .then(async ([nextOverview, nextStatus]) => {
        if (nextStatus.modelVersion === null) {
          await rebuildCareerMmr();
          [nextOverview, nextStatus] = await Promise.all([
            getCareerMmrOverview(),
            getCareerMmrBuildStatus(),
          ]);
        }
        if (!active) return;
        setOverview(nextOverview);
        setBuildStatus(nextStatus);
      })
      .catch((reason) => active && setError(productErrorMessage(reason)))
      .finally(() => active && setLoading(false));
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!overview) return;
    const request = detailRequestGuard.current.begin();
    setDetailLoading(true);
    setError(null);
    setHistory(null);
    setModes(null);
    setSeasons(null);
    Promise.all([
      getRoleMmrHistory({ role: selectedRole, season: selectedSeason, granularity: "match", limit: 80 }),
      getRoleMmrModeBreakdown(selectedRole),
      getRoleMmrSeasonBreakdown(selectedRole),
    ])
      .then(([nextHistory, nextModes, nextSeasons]) => {
        if (!detailRequestGuard.current.isCurrent(request)) return;
        setHistory(nextHistory);
        setModes(nextModes);
        setSeasons(nextSeasons);
      })
      .catch((reason) => {
        if (detailRequestGuard.current.isCurrent(request)) setError(productErrorMessage(reason));
      })
      .finally(() => {
        if (detailRequestGuard.current.isCurrent(request)) setDetailLoading(false);
      });
    return () => detailRequestGuard.current.invalidate(request);
  }, [overview, selectedRole, selectedSeason]);

  const roleSummary = overview?.roles.find((item) => item.role === selectedRole) ?? null;
  const visibleSeasonItems = useMemo(
    () => seasons?.items.filter((item) => selectedSeason === undefined || item.season === selectedSeason) ?? [],
    [seasons, selectedSeason],
  );

  if (loading) return <section className="workspace careerMmrWorkspace"><div className="emptyState">正在计算生涯 MMR…</div></section>;

  return (
    <section className="workspace careerMmrWorkspace">
      {error && <div className="errorBanner phase6RecoverableError"><AlertCircle size={18} /><span>{error}</span></div>}

      <header className="careerMmrHero">
        <div>
          <span className="careerMmrEyebrow"><ShieldCheck size={15} /> 本地生涯评级</span>
          <h2>生涯三职责 MMR</h2>
          <p>{overview?.disclaimer || "这是依据本地对局数据生成的估算，并非暴雪官方隐藏分。"}</p>
        </div>
        <div className="careerMmrComposite">
          <span>综合分</span>
          <strong>{formatCareerMmr(overview?.compositeMmr ?? null)}</strong>
          <small>{buildStatus?.isBuilt
            ? `模型 v${buildStatus.modelVersion ?? "-"} · 已覆盖三职责`
            : buildStatus && buildStatus.summaryCount > 0
              ? `模型 v${buildStatus.modelVersion ?? "-"} · 已生成 ${buildStatus.summaryCount}/3 个职责，等待其余职责数据`
              : "等待本地模型构建"}</small>
        </div>
      </header>

      <div className="careerMmrRoleGrid" aria-label={`${roleNamesForAccessibility}职责 MMR`}>
        {roles.map((role) => {
          const summary = overview?.roles.find((item) => item.role === role);
          return (
            <button key={role} type="button" aria-pressed={selectedRole === role} className={`careerMmrRoleCard ${selectedRole === role ? "active" : ""}`} onClick={() => setSelectedRole(role)}>
              <span>{careerRoleLabels[role]}</span>
              <strong>{formatCareerMmr(summary?.currentMmr ?? null)}</strong>
              <div><span>置信度 {summary?.confidenceLabel ?? "定位中"}</span><b>{formatCareerPercent(summary?.confidence ?? 0)}</b></div>
              <div className="careerMmrConfidence"><i style={{ width: formatCareerPercent(summary?.confidence ?? 0) }} /></div>
              <small>生涯峰值 {formatCareerMmr(summary?.peakMmr ?? null)} · {formatCareerDate(summary?.peakAt ?? null)}</small>
              <small>{summary ? summary.effectiveMatches.toFixed(1) : "0.0"} 有效场 · {summary ? summary.effectiveHours.toFixed(1) : "0.0"} 小时</small>
            </button>
          );
        })}
      </div>

      <div className="careerMmrToolbar">
        <div>
          <b>{careerRoleLabels[selectedRole]}趋势</b>
          <span>筛选只改变图表展示，不会重新计算当前 MMR。</span>
        </div>
        <label><span>赛季</span><select value={selectedSeason ?? ""} onChange={(event) => setSelectedSeason(event.target.value ? Number(event.target.value) : undefined)}>
          <option value="">全部动态赛季</option>
          {overview && overview.discoveredSeasons.map((season) => <option key={season} value={season}>S{season}</option>)}
        </select></label>
      </div>

      <div className="careerMmrMainGrid">
        <article className="careerMmrPanel careerMmrHistoryPanel">
          <div className="careerMmrPanelTitle"><div><TrendingUp size={17} /><b>历史走势</b></div><span>{history?.points.length ?? 0} 个记录点</span></div>
          {detailLoading ? <div className="careerMmrEmpty">正在加载职责历史…</div> : <HistoryChart history={history} />}
        </article>
        <article className="careerMmrPanel">
          <div className="careerMmrPanelTitle"><div><BarChart3 size={17} /><b>模式贡献</b></div></div>
          <div className="careerMmrModeList">
            {modes?.items.length ? modes.items.map((item) => (
              <div key={item.modeFamily} className="careerMmrModeItem">
                <div><b>{careerModeLabels[item.modeFamily] ?? "其他模式"}</b><span>{item.matchesCount} 场 · {Math.round(item.durationSec / 360) / 10} 小时</span></div>
                <strong className={item.cumulativeDelta >= 0 ? "positive" : "negative"}>{signedCareerDelta(item.cumulativeDelta)}</strong>
                <small>{item.wins} 胜 / {item.losses} 负 / {item.draws} 平</small>
              </div>
            )) : <div className="careerMmrEmpty">暂无模式贡献数据</div>}
          </div>
        </article>
      </div>

      <article className="careerMmrPanel careerMmrSeasonPanel">
        <div className="careerMmrPanelTitle"><div><Clock3 size={17} /><b>赛季分解</b></div></div>
        <div className="careerMmrSeasonTable">
          {visibleSeasonItems.map((item) => <div key={item.season}><b>S{item.season}</b><span>{item.matchesCount} 场</span><span>{item.startMmr} → {item.endMmr}</span><strong>{signedCareerDelta(item.netDelta)}</strong></div>)}
          {!visibleSeasonItems.length && <div className="careerMmrEmpty">当前筛选下暂无赛季记录</div>}
        </div>
      </article>

      <footer className="careerMmrFootnote">当前 {careerRoleLabels[selectedRole]}：{formatCareerMmr(roleSummary?.currentMmr ?? null)} · 生涯估算会在本地数据更新后重放全部动态赛季。</footer>
    </section>
  );
}

function HistoryChart({ history }: { history: CareerMmrHistory | null }) {
  const points = history?.points ?? [];
  const width = 760;
  const height = 210;
  const chart = buildCareerMmrChart(points, width, height);
  if (!chart) return <div className="careerMmrEmpty">数据不足，当前职责仍在定位中。</div>;
  const coordinates = chart.line.map((point) => `${point.x},${point.y}`).join(" ");
  const confidenceBand = chart.band.map((point) => `${point.x},${point.y}`).join(" ");
  const latestPoint = points[points.length - 1];
  return <div className="careerMmrChart"><svg viewBox={`0 0 ${width} ${height}`} overflow="hidden" role="img" aria-label={`${careerRoleLabels[history!.role]} MMR 历史走势及置信区间`}><polygon points={confidenceBand} /><polyline points={coordinates} /></svg><div><span>{formatCareerDate(points[0].timestamp)}</span><b>{formatCareerMmr(latestPoint?.mmr ?? null)}</b><span>{formatCareerDate(latestPoint?.timestamp ?? null)}</span></div></div>;
}
