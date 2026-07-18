import { BarChart3, Clock3, TrendingUp } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
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
  buildCareerMmrChartSummary,
  buildCareerMmrYAxisTicks,
  createCareerMmrRequestGuard,
  formatCareerDate,
  formatCareerMmr,
  formatCareerPercent,
  formatCareerMmrTooltip,
  signedCareerDelta,
} from "../shared/careerMmrFormat";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
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
  const navigate = useNavigate();
  const [overview, setOverview] = useState<CareerMmrOverview | null>(null);
  const [selectedRole, setSelectedRole] = useState<CareerRole>("tank");
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>();
  const [history, setHistory] = useState<CareerMmrHistory | null>(null);
  const [modes, setModes] = useState<CareerMmrModeBreakdown | null>(null);
  const [seasons, setSeasons] = useState<CareerMmrSeasonBreakdown | null>(null);
  const [buildStatus, setBuildStatus] = useState<CareerMmrBuildStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [overviewError, setOverviewError] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [overviewRetry, setOverviewRetry] = useState(0);
  const [detailRetry, setDetailRetry] = useState(0);
  const detailRequestGuard = useRef(createCareerMmrRequestGuard());

  useEffect(() => {
    let active = true;
    setLoading(true);
    setOverviewError(null);
    Promise.all([getCareerMmrOverview(), getCareerMmrBuildStatus()])
      .then(async ([nextOverview, nextStatus]) => {
        if (nextStatus.needsRebuild) {
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
      .catch((reason) => active && setOverviewError(productErrorMessage(reason)))
      .finally(() => active && setLoading(false));
    return () => { active = false; };
  }, [overviewRetry]);

  useEffect(() => {
    if (!overview || overview.roles.length === 0) return;
    const request = detailRequestGuard.current.begin();
    setDetailLoading(true);
    setDetailError(null);
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
        if (detailRequestGuard.current.isCurrent(request)) setDetailError(productErrorMessage(reason));
      })
      .finally(() => {
        if (detailRequestGuard.current.isCurrent(request)) setDetailLoading(false);
      });
    return () => detailRequestGuard.current.invalidate(request);
  }, [overview, selectedRole, selectedSeason, detailRetry]);

  const roleSummary = overview?.roles.find((item) => item.role === selectedRole) ?? null;
  const visibleSeasonItems = useMemo(
    () => seasons?.items.filter((item) => selectedSeason === undefined || item.season === selectedSeason) ?? [],
    [seasons, selectedSeason],
  );

  const pageHeader = (
    <PageHeader
      title="生涯 MMR"
      description={overview?.disclaimer || "本地模型推算，并非暴雪官方隐藏分"}
      actions={
        <PageSeasonSelect
          label="展示赛季"
          value={selectedSeason}
          seasons={overview?.discoveredSeasons ?? []}
          includeAll
          allLabel="全部"
          onChange={setSelectedSeason}
        />
      }
    />
  );

  if (loading) {
    return (
      <section className="workspace careerMmrWorkspace">
        {pageHeader}
        <PageLoadingState message="正在计算生涯 MMR…" />
      </section>
    );
  }

  if (overviewError) {
    return (
      <section className="workspace careerMmrWorkspace">
        {pageHeader}
        <PageErrorState message={overviewError} onRetry={() => setOverviewRetry((value) => value + 1)} />
      </section>
    );
  }

  if (!overview || overview.roles.length === 0) {
    return (
      <section className="workspace careerMmrWorkspace">
        {pageHeader}
        <PageEmptyState
          title="暂无生涯 MMR"
          description="当前本地数据还不足以生成职责估算，请先在指挥台同步赛季数据。"
          actionLabel="前往指挥台"
          onAction={() => navigate("/onboarding#local-data")}
        />
      </section>
    );
  }

  return (
    <section className="workspace careerMmrWorkspace">
      {pageHeader}

      <section className="careerMmrSummary" aria-label="综合摘要">
        <div className="careerMmrComposite">
          <span>综合摘要</span>
          <strong>{formatCareerMmr(overview?.compositeMmr ?? null)}</strong>
          <small>{buildStatus?.isBuilt
            ? `模型 v${buildStatus.modelVersion ?? "-"} · 已覆盖三职责`
            : buildStatus && buildStatus.summaryCount > 0
              ? `模型 v${buildStatus.modelVersion ?? "-"} · 已生成 ${buildStatus.summaryCount}/3 个职责，等待其余职责数据`
              : "等待本地模型构建"}</small>
        </div>
        <p>三职责分别保留当前估算、置信度、峰值和有效样本；职责选择只改变下方趋势与分解。</p>
      </section>

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

      <div className="careerMmrSectionIntro">
        <div>
          <b>{careerRoleLabels[selectedRole]}趋势</b>
          <span>展示赛季只改变趋势和分解，不会重新计算当前 MMR。</span>
        </div>
      </div>

      <div className="careerMmrMainGrid">
        <article className="careerMmrPanel careerMmrHistoryPanel">
          <div className="careerMmrPanelTitle"><div><TrendingUp size={17} /><b>历史走势</b></div><span>{history?.points.length ?? 0} 个记录点</span></div>
          {detailError ? (
            <PageErrorState message={detailError} onRetry={() => setDetailRetry((value) => value + 1)} />
          ) : detailLoading ? (
            <div className="careerMmrEmpty">正在加载职责历史…</div>
          ) : (
            <HistoryChart history={history} />
          )}
        </article>
        <article className="careerMmrPanel">
          <div className="careerMmrPanelTitle"><div><BarChart3 size={17} /><b>模式贡献</b></div></div>
          <div className="careerMmrModeList">
            {modes?.items.length ? modes.items.map((item) => (
              <div key={item.modeFamily} className="careerMmrModeItem">
                <div><b>{careerModeLabels[item.modeFamily] ?? "其他模式"}</b><span>{item.matchesCount} 场 · {Math.round(item.durationSec / 360) / 10} 小时</span></div>
                <div className="careerMmrModeValue">
                  <span>生涯累计贡献</span>
                  <strong className={item.cumulativeDelta >= 0 ? "positive" : "negative"}>{signedCareerDelta(item.cumulativeDelta)}</strong>
                </div>
                <small>{item.wins} 胜 / {item.losses} 负 / {item.draws} 平</small>
              </div>
            )) : <div className="careerMmrEmpty">暂无模式贡献数据</div>}
          </div>
        </article>
      </div>

      <article className="careerMmrPanel careerMmrSeasonPanel">
        <div className="careerMmrPanelTitle"><div><Clock3 size={17} /><b>赛季分解</b></div></div>
        <div className="careerMmrSeasonTable">
          {visibleSeasonItems.map((item) => <div key={item.season}><b>S{item.season}</b><span>{item.matchesCount} 场</span><span>{formatCareerMmr(item.startMmr)} → {formatCareerMmr(item.endMmr)}</span><strong className={item.netDelta >= 0 ? "positive" : "negative"}>{signedCareerDelta(item.netDelta)}</strong></div>)}
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
  const height = 250;
  const left = 64;
  const top = 16;
  const right = 20;
  const bottom = 40;
  const plotWidth = width - left - right;
  const plotHeight = height - top - bottom;
  const chart = buildCareerMmrChart(points, plotWidth, plotHeight);
  if (!chart) return <div className="careerMmrEmpty">数据不足，当前职责仍在定位中。</div>;
  const coordinates = chart.line.map((point) => `${point.x},${point.y}`).join(" ");
  const confidenceBand = chart.band.map((point) => `${point.x},${point.y}`).join(" ");
  const confidenceUpperCoordinates = chart.band.slice(0, chart.line.length).map((point) => `${point.x},${point.y}`).join(" ");
  const confidenceLowerCoordinates = chart.band.slice(chart.line.length).reverse().map((point) => `${point.x},${point.y}`).join(" ");
  const baselineY = chart.line[0].y;
  const baselineMmr = points[chart.line[0].inputIndex].mmr;
  const yTicks = buildCareerMmrYAxisTicks(chart.domainMin, chart.domainMax, 4);
  const yForValue = (value: number) => top + plotHeight - ((value - chart.domainMin) / (chart.domainMax - chart.domainMin)) * plotHeight;
  const xTickIndexes = [...new Set([0, Math.floor((chart.line.length - 1) / 2), chart.line.length - 1])];

  return (
    <div className="careerMmrChart">
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`${careerRoleLabels[history!.role]} MMR 历史走势、置信区间上下界与起点基准`}>
        <g className="careerMmrYAxis" aria-label="MMR 纵轴">
          <text x="8" y="12">MMR</text>
          {yTicks.map((tick) => {
            const y = yForValue(tick);
            return (
              <g key={tick}>
                <line x1={left} x2={width - right} y1={y} y2={y} />
                <text x={left - 10} y={y + 4}>{formatCareerMmr(tick)}</text>
              </g>
            );
          })}
        </g>
        <g className="careerMmrPlot" transform={`translate(${left} ${top})`}>
          <polygon points={confidenceBand} />
          <line className="careerMmrBaseline" x1="0" x2={plotWidth} y1={baselineY} y2={baselineY} />
          <polyline className="careerMmrConfidenceBound careerMmrConfidenceUpper" points={confidenceUpperCoordinates} />
          <polyline className="careerMmrConfidenceBound careerMmrConfidenceLower" points={confidenceLowerCoordinates} />
          <polyline className="careerMmrEstimateLine" points={coordinates} />
          {chart.line.map((coordinate) => {
            const point = points[coordinate.inputIndex];
            return (
              <circle
                key={`${point.timestamp}-${coordinate.inputIndex}`}
                cx={coordinate.x}
                cy={coordinate.y}
                r="8"
                tabIndex={0}
              >
                <title>{formatCareerMmrTooltip(point)}</title>
              </circle>
            );
          })}
        </g>
        <g className="careerMmrXAxis" aria-label="日期横轴">
          {xTickIndexes.map((index) => {
            const coordinate = chart.line[index];
            const point = points[coordinate.inputIndex];
            return (
              <text key={`${point.timestamp}-${index}`} x={left + coordinate.x} y={height - 8} textAnchor="middle">
                {formatCareerDate(point.timestamp)}
              </text>
            );
          })}
        </g>
      </svg>
      <div className="careerMmrChartLegend" aria-label="图表图例">
        <span><i className="careerMmrLegendEstimate" aria-hidden="true" />MMR 估算</span>
        <span><i className="careerMmrLegendConfidence" aria-hidden="true" />置信区间上下界</span>
        <span><i className="careerMmrLegendBaseline" aria-hidden="true" />起点基准：{formatCareerMmr(baselineMmr)} MMR</span>
      </div>
      <p className="careerMmrScreenReaderSummary">
        {buildCareerMmrChartSummary(history!.role, points)}
      </p>
    </div>
  );
}
