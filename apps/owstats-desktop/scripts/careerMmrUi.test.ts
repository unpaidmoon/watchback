import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  buildCareerMmrChart,
  buildCareerMmrChartSummary,
  buildCareerMmrYAxisTicks,
  createCareerMmrRequestGuard,
  formatCareerMmrTooltip,
} from "../src/shared/careerMmrFormat";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const shellSource = readFileSync(new URL("../src/shared/phase6Shell.ts", import.meta.url), "utf8");
const pageSource = readFileSync(new URL("../src/pages/CareerMmrPage.tsx", import.meta.url), "utf8");
const apiSource = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const typeSource = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");
const styleSource = readFileSync(new URL("../src/styles/features/career-mmr.css", import.meta.url), "utf8");
const tokenSource = readFileSync(new URL("../src/styles/tokens.css", import.meta.url), "utf8");

function tokenHex(name: string) {
  const match = tokenSource.match(new RegExp(`--${name}:\\s*(#[0-9a-f]{6})`, "i"));
  assert.ok(match, `缺少 ${name} 颜色令牌`);
  return match[1];
}

function relativeLuminance(hex: string) {
  const channels = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255);
  const [red, green, blue] = channels.map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function contrastRatio(foreground: string, background: string) {
  const lighter = Math.max(relativeLuminance(foreground), relativeLuminance(background));
  const darker = Math.min(relativeLuminance(foreground), relativeLuminance(background));
  return (lighter + 0.05) / (darker + 0.05);
}

assert.match(appSource, /path="\/career-mmr"/, "生涯 MMR 页面必须注册独立路由");
assert.match(shellSource, /生涯 MMR/, "主导航必须提供生涯 MMR 入口");
assert.match(pageSource, /并非暴雪官方隐藏分/, "页面必须清楚声明这是本地估算");
assert.match(pageSource, /重装/);
assert.match(pageSource, /输出/);
assert.match(pageSource, /支援/);
assert.doesNotMatch(pageSource, /S21.*S22.*S23/s, "赛季选项不得硬编码");
assert.match(pageSource, /seasons=\{overview\?\.discoveredSeasons \?\? \[\]\}/, "赛季筛选必须来自后端动态发现结果");
assert.match(pageSource, /getCareerMmrOverview\(/, "页面必须通过 API 获取总览");
assert.match(pageSource, /getRoleMmrHistory\(/, "页面必须通过 API 获取历史");
assert.doesNotMatch(pageSource, /getRecentMmrExplanations/);
assert.doesNotMatch(pageSource, /recentExplanations/);
assert.doesNotMatch(pageSource, /近期单场解释/);
assert.doesNotMatch(pageSource, /CareerMmrRecentExplanations/);
assert.match(pageSource, /Promise\.all\(\[\s*getRoleMmrHistory[\s\S]*getRoleMmrModeBreakdown[\s\S]*getRoleMmrSeasonBreakdown[\s\S]*\]\)/);
assert.match(pageSource, /className="careerMmrPanel careerMmrSeasonPanel"/);
assert.doesNotMatch(pageSource, /careerMmrLowerGrid/);
assert.match(pageSource, /formatCareerDate\(summary\?\.peakAt/, "职责卡必须展示后端峰值日期");
assert.match(apiSource, /invoke<CareerMmrOverview>\("get_career_mmr_overview"\)/);
assert.match(apiSource, /invoke<CareerMmrHistory>\("get_role_mmr_history"/);
assert.match(apiSource, /invoke\("rebuild_career_mmr"\)/, "前端必须能够触发隐私安全的本地 MMR 重建");
assert.match(pageSource, /nextStatus\.modelVersion === null[\s\S]*rebuildCareerMmr\(\)/, "首次打开旧库升级后的页面必须自动重建一次 MMR");
assert.match(typeSource, /export type CareerRole = "tank" \| "damage" \| "support"/);
assert.match(typeSource, /peakAt: number \| null/);
assert.doesNotMatch(pageSource, /胜负、对局难度与数据质量共同形成/, "不能用通用文案伪装单场解释");
assert.doesNotMatch(pageSource, /cursorHash/, "页面不能读取或展示内部游标哈希");
assert.doesNotMatch(pageSource, /matchId|playerId|bnetId|rawJson/, "页面不能读取或展示隐私标识");
assert.match(pageSource, /aria-pressed=\{selectedRole === role\}/, "职责按钮必须暴露选中状态");
assert.match(pageSource, /setHistory\(null\);[\s\S]*setModes\(null\);[\s\S]*setSeasons\(null\)/, "切换职责必须立即清空旧详情");
assert.match(pageSource, /aria-label="MMR 纵轴"/);
assert.match(pageSource, /<title>\{formatCareerMmrTooltip\(point\)\}<\/title>/);
assert.match(pageSource, /className="careerMmrScreenReaderSummary"/);
assert.match(pageSource, /className=\{item\.netDelta >= 0 \? "positive" : "negative"\}/, "赛季净变化必须同时用正负号和语义色表达状态");
assert.match(pageSource, /className="careerMmrConfidenceBound careerMmrConfidenceUpper"/, "置信上界必须使用真实区间坐标绘制冷蓝绿色辅线");
assert.match(pageSource, /className="careerMmrConfidenceBound careerMmrConfidenceLower"/, "置信下界必须使用真实区间坐标绘制冷蓝绿色辅线");
assert.match(pageSource, /className="careerMmrBaseline"/, "趋势图必须显示所选序列的灰色起点基准线");
assert.match(pageSource, /aria-label="图表图例"[\s\S]*MMR 估算[\s\S]*置信区间上下界[\s\S]*起点基准/, "图表必须用中文文本解释三类线条的真实语义");
assert.match(styleSource, /\.careerMmrConfidenceBound\s*{[\s\S]*?stroke:\s*var\(--data-secondary\)/, "置信边界辅线必须使用冷蓝绿色数据令牌");
assert.match(styleSource, /\.careerMmrBaseline\s*{[\s\S]*?stroke:\s*var\(--data-neutral-strong\)/, "起点基准必须使用可辨认的灰色数据令牌");
assert.match(
  styleSource,
  /\.careerMmrPanel\s*\{[^}]*border:\s*1px solid var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);[^}]*box-shadow:\s*none;/s,
  "MMR 面板应使用单层玻璃表面且不叠加阴影",
);
assert.match(
  styleSource,
  /\.careerMmrRoleCard\s*\{[^}]*border:\s*1px solid var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);[^}]*box-shadow:\s*none;/s,
  "职责卡应使用低边界玻璃表面",
);
assert.match(styleSource, /\.careerMmrRoleCard > span\s*\{[^}]*font-weight:\s*400;/s, "职责说明应使用常规字重");
assert.match(pageSource, /生涯累计贡献/);
assert.match(pageSource, /label="展示赛季"/);
assert.match(pageSource, /<PageHeader/);
assert.match(pageSource, /<PageSeasonSelect/);
assert.match(pageSource, /<PageLoadingState/);
assert.match(pageSource, /<PageErrorState/);
assert.match(pageSource, /<PageEmptyState/);
assert.match(
  pageSource,
  /if \(!overview \|\| overview\.roles\.length === 0\) return;/,
  "an empty overview should not trigger role-detail requests",
);
assert.match(
  pageSource,
  /title="暂无生涯 MMR"[\s\S]*actionLabel="前往指挥台"[\s\S]*navigate\("\/onboarding#local-data"\)/,
  "the empty state should explain missing MMR data and offer one useful recovery action",
);
assert.doesNotMatch(styleSource, /gradient/i, "生涯 MMR 不得保留蓝色渐变");
assert.doesNotMatch(styleSource, /--phase6-|#6d8cff|#65d6c0/i, "生涯 MMR 必须只使用语义视觉令牌");
assert.match(styleSource, /\.careerMmrModeItem \.positive[\s\S]*?color:\s*var\(--data-secondary-strong\)/, "小号正值必须使用高对比冷蓝绿色令牌");
assert.match(styleSource, /\.careerMmrModeItem \.negative[\s\S]*?color:\s*var\(--status-danger-strong\)/, "小号负值必须使用高对比危险色令牌");

for (const foreground of [tokenHex("data-secondary-strong"), tokenHex("status-danger-strong")]) {
  for (const background of [tokenHex("surface-secondary"), tokenHex("surface-control")]) {
    assert.ok(contrastRatio(foreground, background) >= 4.5, `${foreground} 在 ${background} 上的正文对比度必须至少为 4.5:1`);
  }
}
assert.ok(
  contrastRatio(tokenHex("data-neutral-strong"), tokenHex("surface-secondary")) >= 3,
  "灰色基准线与图表背景的对比度必须至少为 3:1",
);

assert.equal(buildCareerMmrChart([], 760, 210), null, "空历史不应构造图表");
const flat = buildCareerMmrChart([{ mmr: 2500, confidence: 1 }, { mmr: 2500, confidence: 1 }], 760, 210)!;
assert.ok(flat.line.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y)), "平坦历史坐标必须有限");
const uncertain = buildCareerMmrChart([{ mmr: 2500, confidence: 0 }, { mmr: 2550, confidence: 0.1 }], 760, 210)!;
assert.ok(uncertain.band.every((point) => point.y >= 0 && point.y <= 210), "低置信区间必须约束在 viewport 内");
assert.ok(uncertain.domainMin <= 2380 && uncertain.domainMax >= 2620, "纵轴范围必须纳入置信区间上下界");

const chartPoints = [
  { timestamp: 1_720_000_000, season: 22, mmr: 2480, confidence: 0.7, delta: -12.5 },
  { timestamp: 1_720_086_400, season: 22, mmr: 2525, confidence: 0.8, delta: 45.0 },
];
const chart = buildCareerMmrChart(chartPoints, 760, 210)!;
const ticks = buildCareerMmrYAxisTicks(chart.domainMin, chart.domainMax, 4);
assert.equal(ticks.length, 4);
assert.ok(ticks.every((value) => Number.isFinite(value)));
assert.match(formatCareerMmrTooltip(chartPoints[1]), /S22/);
assert.match(formatCareerMmrTooltip(chartPoints[1]), /2,525/);
assert.match(buildCareerMmrChartSummary("damage", chartPoints), /输出/);
assert.match(buildCareerMmrChartSummary("damage", chartPoints), /上升 45/);

const guard = createCareerMmrRequestGuard();
const first = guard.begin();
const second = guard.begin();
assert.equal(guard.isCurrent(first), false, "旧职责请求不得覆盖新职责");
assert.equal(guard.isCurrent(second), true, "最新职责请求可以更新页面");
guard.invalidate(second);
assert.equal(guard.isCurrent(second), false, "卸载时必须使在途请求失效");

console.log("career MMR UI contracts passed");
