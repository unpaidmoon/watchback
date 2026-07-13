import type { CareerRole } from "./types";

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
type CareerChartPoint = { x: number; y: number };

export function buildCareerMmrChart(inputs: CareerChartInput[], width: number, height: number) {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return null;
  const points = inputs.filter((point) => Number.isFinite(point.mmr) && Number.isFinite(point.confidence));
  if (!points.length) return null;
  const bounds = points.flatMap((point) => {
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
  const line = points.map((point, index) => ({ x: x(index), y: y(point.mmr) }));
  const upper = points.map((point, index) => ({
    x: x(index),
    y: y(point.mmr + (1 - Math.max(0, Math.min(1, point.confidence))) * 120),
  }));
  const lower = points.map((point, index) => ({
    x: x(index),
    y: y(point.mmr - (1 - Math.max(0, Math.min(1, point.confidence))) * 120),
  })).reverse();
  return { line, band: [...upper, ...lower] as CareerChartPoint[], domainMin, domainMax };
}

export function createCareerMmrRequestGuard() {
  let generation = 0;
  return {
    begin() { generation += 1; return generation; },
    isCurrent(request: number) { return generation === request; },
    invalidate(request?: number) { if (request === undefined || generation === request) generation += 1; },
  };
}
