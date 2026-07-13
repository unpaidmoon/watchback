import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { buildCareerMmrChart, createCareerMmrRequestGuard } from "../src/shared/careerMmrFormat";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const shellSource = readFileSync(new URL("../src/shared/phase6Shell.ts", import.meta.url), "utf8");
const pageSource = readFileSync(new URL("../src/pages/CareerMmrPage.tsx", import.meta.url), "utf8");
const apiSource = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const typeSource = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");

assert.match(appSource, /path="\/career-mmr"/, "生涯 MMR 页面必须注册独立路由");
assert.match(shellSource, /生涯 MMR/, "主导航必须提供生涯 MMR 入口");
assert.match(pageSource, /并非暴雪官方隐藏分/, "页面必须清楚声明这是本地估算");
assert.match(pageSource, /重装/);
assert.match(pageSource, /输出/);
assert.match(pageSource, /支援/);
assert.doesNotMatch(pageSource, /S21.*S22.*S23/s, "赛季选项不得硬编码");
assert.match(pageSource, /overview\.discoveredSeasons\.map/, "赛季筛选必须来自后端动态发现结果");
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

assert.equal(buildCareerMmrChart([], 760, 210), null, "空历史不应构造图表");
const flat = buildCareerMmrChart([{ mmr: 2500, confidence: 1 }, { mmr: 2500, confidence: 1 }], 760, 210)!;
assert.ok(flat.line.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y)), "平坦历史坐标必须有限");
const uncertain = buildCareerMmrChart([{ mmr: 2500, confidence: 0 }, { mmr: 2550, confidence: 0.1 }], 760, 210)!;
assert.ok(uncertain.band.every((point) => point.y >= 0 && point.y <= 210), "低置信区间必须约束在 viewport 内");
assert.ok(uncertain.domainMin <= 2380 && uncertain.domainMax >= 2620, "纵轴范围必须纳入置信区间上下界");

const guard = createCareerMmrRequestGuard();
const first = guard.begin();
const second = guard.begin();
assert.equal(guard.isCurrent(first), false, "旧职责请求不得覆盖新职责");
assert.equal(guard.isCurrent(second), true, "最新职责请求可以更新页面");
guard.invalidate(second);
assert.equal(guard.isCurrent(second), false, "卸载时必须使在途请求失效");

console.log("career MMR UI contracts passed");
