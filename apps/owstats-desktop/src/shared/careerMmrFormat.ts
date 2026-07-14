import type { CareerMmrHistoryPoint, CareerRole } from "./types";

export const careerRoleLabels: Record<CareerRole, string> = {
  tank: "重装",
  damage: "输出",
  support: "支援",
};

export const careerModeLabels: Record<string, string> = {
  role_competitive: "职责竞技",
  open_competitive: "开放竞技",
  stadium_competitive: "角斗竞技",
  quick_play: "快速比赛",
};

export function formatCareerMmr(value: number | null) {
  return value === null ? "定位中" : Math.round(value).toLocaleString("zh-CN");
}

export function formatCareerPercent(value: number) {
  return `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
}

export function formatCareerDate(timestamp: number | null) {
  if (!timestamp) return "暂无记录";
  return new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "short", day: "numeric" }).format(new Date(timestamp * 1000));
}

export function signedCareerDelta(value: number) {
  if (Math.abs(value) < 0.005) return "±0.0";
  return `${value > 0 ? "+" : ""}${value.toFixed(1)}`;
}

type CareerChartInput = { mmr: number; confidence: number };
type CareerChartPoint = { x: number; y: number; inputIndex: number };
type CareerChartBandPoint = { x: number; y: number };

export function buildCareerMmrChart(inputs: CareerChartInput[], width: number, height: number) {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return null;
  const points = inputs
    .map((point, inputIndex) => ({ point, inputIndex }))
    .filter(({ point }) => Number.isFinite(point.mmr) && Number.isFinite(point.confidence));
  if (!points.length) return null;
  const bounds = points.flatMap(({ point }) => {
    const confidence = Math.max(0, Math.min(1, point.confidence));
    const radius = (1 - confidence) * 120;
    return [point.mmr - radius, point.mmr + radius];
  });
  let domainMin = Math.min(...bounds);
  let domainMax = Math.max(...bounds);
  if (domainMax - domainMin < 80) {
    const midpoint = (domainMin + domainMax) / 2;
    domainMin = midpoint - 40;
    domainMax = midpoint + 40;
  }
  const span = domainMax - domainMin;
  const x = (index: number) => points.length === 1 ? width / 2 : (index / (points.length - 1)) * width;
  const y = (value: number) => Math.max(0, Math.min(height, height - ((value - domainMin) / span) * height));
  const line = points.map(({ point, inputIndex }, index) => ({ x: x(index), y: y(point.mmr), inputIndex }));
  const upper = points.map(({ point }, index) => ({
    x: x(index),
    y: y(point.mmr + (1 - Math.max(0, Math.min(1, point.confidence))) * 120),
  }));
  const lower = points.map(({ point }, index) => ({
    x: x(index),
    y: y(point.mmr - (1 - Math.max(0, Math.min(1, point.confidence))) * 120),
  })).reverse();
  return { line: line as CareerChartPoint[], band: [...upper, ...lower] as CareerChartBandPoint[], domainMin, domainMax };
}

export function buildCareerMmrYAxisTicks(domainMin: number, domainMax: number, count: number) {
  if (!Number.isFinite(domainMin) || !Number.isFinite(domainMax)) return [0];
  if (count < 2) return [Math.round(domainMin)];
  const step = (domainMax - domainMin) / (count - 1);
  return Array.from({ length: count }, (_, index) => Math.round(domainMax - index * step));
}

export function formatCareerMmrTooltip(point: CareerMmrHistoryPoint) {
  return `S${point.season} · ${formatCareerDate(point.timestamp)} · MMR ${formatCareerMmr(point.mmr)} · 变化 ${signedCareerDelta(point.delta)}`;
}

export function buildCareerMmrChartSummary(role: CareerRole, points: CareerMmrHistoryPoint[]) {
  const validPoints = points.filter((point) => Number.isFinite(point.mmr));
  if (!validPoints.length) return `${careerRoleLabels[role]}暂无历史记录。`;
  const first = validPoints[0];
  const last = validPoints[validPoints.length - 1];
  const change = Math.round(last.mmr - first.mmr);
  const direction = change > 0 ? `上升 ${change}` : change < 0 ? `下降 ${Math.abs(change)}` : "保持不变";
  return `${careerRoleLabels[role]}共有 ${validPoints.length} 个记录点，MMR 从 ${Math.round(first.mmr)} 到 ${Math.round(last.mmr)}，${direction}。`;
}

export function createCareerMmrRequestGuard() {
  let generation = 0;
  return {
    begin() { generation += 1; return generation; },
    isCurrent(request: number) { return generation === request; },
    invalidate(request?: number) { if (request === undefined || generation === request) generation += 1; },
  };
}
