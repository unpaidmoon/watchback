# 角斗玩家回合汇总卡片 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在角斗汇总中点击任意玩家英雄头像，打开纯前端计算的跨回合汇总弹窗，展示 R1–R7 的回合开始资金及回合结束时完整异能、装备。

**Architecture:** 新建独立的纯计算模块和展示弹窗，由 `CompactArenaDetail` 持有当前目标玩家状态。`MatchTeamTable` 只增加可选的头像操作入口，并将头像按钮与整行展开按钮做成并列控件，避免嵌套交互元素。

**Tech Stack:** React 18、TypeScript、现有 Node/tsx 断言测试、CSS、现有英雄与角斗资产解析器。

---

## 文件结构

- Create: `apps/owstats-desktop/src/features/matches/arenaPlayerRoundSummary.ts`
  - 定义汇总行类型、跨回合玩家关联与资金错位计算。
- Create: `apps/owstats-desktop/src/features/matches/ArenaPlayerRoundSummaryDialog.tsx`
  - 渲染居中弹窗、标题栏、固定槽位、关闭和焦点管理。
- Create: `apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx`
  - 覆盖纯计算、缺失数据和弹窗静态结构。
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
  - 仅在角斗汇总注入头像入口并持有弹窗目标状态。
- Modify: `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`
  - 增加头像操作回调，将头像按钮与整行展开按钮拆成并列控件。
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
  - 增加入口、弹窗、回合行、固定槽位和响应式样式。
- Modify: `apps/owstats-desktop/package.json`
  - 将新测试加入 `test:components`。
- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`
  - 更新玩家行交互结构的源码契约断言。

### Task 1: 用测试锁定跨回合计算规则

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/arenaPlayerRoundSummary.ts`
- Create: `apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx`

- [ ] **Step 1: 创建失败的计算测试**

在 `arenaPlayerRoundSummary.test.tsx` 中构造不含真实 ID 的合成玩家与三个回合，并写入以下断言：

```tsx
import assert from "node:assert/strict";
import type { ArenaDetailSlice, PlayerRow } from "../src/shared/types";
import { buildArenaPlayerRoundSummary } from "../src/features/matches/arenaPlayerRoundSummary";

function player(overrides: Partial<PlayerRow> = {}): PlayerRow {
  return {
    side: "teammate",
    name: "测试玩家",
    bnetId: "synthetic-player",
    rankLabel: null,
    rankKey: null,
    heroGuid: "synthetic-hero",
    heroName: "测试英雄",
    kills: 0,
    assists: 0,
    deaths: 0,
    damage: 0,
    healing: 0,
    mitigation: 0,
    finalHits: 0,
    targetTimeSec: 0,
    healingTaken: 0,
    damageTaken: 0,
    worth: null,
    endWorth: null,
    worthMax: false,
    modGuids: [],
    traitGuids: [],
    hasSourceMaxFlags: false,
    killMax: false,
    heroDamageMax: false,
    cureMax: false,
    resistDamageMax: false,
    perks: [],
    heroStats: [],
    ...overrides,
  };
}

function round(key: string, teammates: PlayerRow[]): ArenaDetailSlice {
  return {
    key,
    label: key,
    result: null,
    resultLabel: null,
    teamScore: null,
    opponentScore: null,
    durationSec: null,
    beginTs: null,
    beginLabel: null,
    mapGuid: null,
    mapName: null,
    teammates,
    enemies: [],
  };
}

const target = player();
const rows = buildArenaPlayerRoundSummary(target, [
  round("round-1", [player({ worth: 3000, endWorth: 4200, traitGuids: ["trait-a"], modGuids: ["mod-a"] })]),
  round("round-2", [player({ worth: 4200, endWorth: 6100, traitGuids: ["trait-a", "trait-b"], modGuids: ["mod-a", "mod-b"] })]),
  round("round-3", [player({ worth: 6100, endWorth: 8000, traitGuids: ["trait-a", "trait-b", "trait-c"], modGuids: ["mod-a", "mod-b", "mod-c"] })]),
]);

assert.deepEqual(rows.map((row) => row.label), ["R1", "R2", "R3"]);
assert.deepEqual(rows.map((row) => row.startWorth), [3000, 4200, 6100]);
assert.deepEqual(rows[1]?.traitGuids, ["trait-a", "trait-b"]);
assert.deepEqual(rows[1]?.modGuids, ["mod-a", "mod-b"]);

const fallbackRows = buildArenaPlayerRoundSummary(target, [
  round("round-1", [player({ endWorth: null })]),
  round("round-2", [player({ worth: 4700, endWorth: 6000 })]),
  round("round-3", [player({ worth: null, endWorth: 7200 })]),
]);
assert.deepEqual(fallbackRows.map((row) => row.startWorth), [3000, 4700, 6000]);

const missingRows = buildArenaPlayerRoundSummary(target, [
  round("round-1", [player({ endWorth: 3900 })]),
  round("round-2", []),
  round("round-3", [player({ worth: null, endWorth: 6500 })]),
]);
assert.equal(missingRows.length, 3);
assert.equal(missingRows[1]?.startWorth, 3900);
assert.deepEqual(missingRows[1]?.traitGuids, []);
assert.equal(missingRows[2]?.startWorth, null);
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
```

Expected: FAIL，提示找不到 `arenaPlayerRoundSummary` 模块或导出。

- [ ] **Step 3: 实现最小纯计算模块**

创建 `arenaPlayerRoundSummary.ts`：

```ts
import type { ArenaDetailSlice, PlayerRow } from "../../shared/types";

export type ArenaPlayerRoundSummaryRow = {
  key: string;
  label: string;
  startWorth: number | null;
  traitGuids: string[];
  modGuids: string[];
};

export function buildArenaPlayerRoundSummary(
  target: PlayerRow,
  rounds: ArenaDetailSlice[],
): ArenaPlayerRoundSummaryRow[] {
  return rounds.map((round, index) => {
    const current = findPlayer(round, target);
    const previous = index > 0 ? findPlayer(rounds[index - 1], target) : null;
    return {
      key: round.key,
      label: `R${index + 1}`,
      startWorth: index === 0 ? 3000 : previous?.endWorth ?? current?.worth ?? null,
      traitGuids: current?.traitGuids.slice(0, 4) ?? [],
      modGuids: current?.modGuids.slice(0, 6) ?? [],
    };
  });
}

function findPlayer(round: ArenaDetailSlice, target: PlayerRow): PlayerRow | null {
  const players = [...round.teammates, ...round.enemies];
  if (target.bnetId) {
    const byId = players.find((candidate) => candidate.bnetId === target.bnetId);
    if (byId) return byId;
  }
  return players.find((candidate) => candidate.name === target.name) ?? null;
}
```

- [ ] **Step 4: 运行计算测试确认通过**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
```

Expected: PASS，进程退出码为 0。

- [ ] **Step 5: 提交计算单元**

```powershell
git add apps/owstats-desktop/src/features/matches/arenaPlayerRoundSummary.ts apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx
git commit -m "feat: calculate stadium player round summaries"
```

### Task 2: 实现无注释的汇总弹窗

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/ArenaPlayerRoundSummaryDialog.tsx`
- Modify: `apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx`

- [ ] **Step 1: 添加弹窗静态结构失败测试**

在测试文件中增加：

```tsx
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ArenaPlayerRoundSummaryDialog } from "../src/features/matches/ArenaPlayerRoundSummaryDialog";

const dialogMarkup = renderToStaticMarkup(createElement(ArenaPlayerRoundSummaryDialog, {
  player: target,
  rows,
  onClose: () => undefined,
}));

assert.match(dialogMarkup, /role="dialog"/);
assert.match(dialogMarkup, /aria-modal="true"/);
assert.match(dialogMarkup, />汇总卡片</);
assert.match(dialogMarkup, />测试玩家</);
assert.match(dialogMarkup, />R1</);
assert.match(dialogMarkup, />3000</);
assert.equal((dialogMarkup.match(/phase7SummaryTraitSlot/g) ?? []).length, rows.length * 4);
assert.equal((dialogMarkup.match(/phase7SummaryModSlot/g) ?? []).length, rows.length * 6);
assert.doesNotMatch(dialogMarkup, /phase6PerkTooltipPanel/);
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
```

Expected: FAIL，提示找不到 `ArenaPlayerRoundSummaryDialog`。

- [ ] **Step 3: 创建弹窗组件**

实现 `ArenaPlayerRoundSummaryDialog.tsx`。组件必须使用 `useEffect` 和 `useRef`：挂载时聚焦关闭按钮并监听 `Escape`；卸载时移除监听。遮罩只在 `event.target === event.currentTarget` 时关闭，避免内容点击误关。

```tsx
import { X } from "lucide-react";
import { useEffect, useRef } from "react";
import { resolveArenaModTraitInfo } from "../../shared/assets";
import { HeroAvatar } from "../../shared/phase6Components";
import type { PlayerRow } from "../../shared/types";
import type { ArenaPlayerRoundSummaryRow } from "./arenaPlayerRoundSummary";

export function ArenaPlayerRoundSummaryDialog({
  player,
  rows,
  onClose,
}: {
  player: PlayerRow;
  rows: ArenaPlayerRoundSummaryRow[];
  onClose: () => void;
}) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    closeRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="phase7SummaryDialogBackdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section className="phase7SummaryDialog" role="dialog" aria-modal="true" aria-labelledby="phase7-summary-title">
        <header>
          <div>
            <h2 id="phase7-summary-title">汇总卡片</h2>
            <span>{player.name}</span>
          </div>
          <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
          <button ref={closeRef} type="button" onClick={onClose} aria-label="关闭汇总卡片">
            <X size={20} aria-hidden="true" />
          </button>
        </header>
        <div className="phase7SummaryDialogBody">
          {rows.map((row) => (
            <div className="phase7SummaryRound" key={row.key}>
              <strong>{row.label}</strong>
              <span className="phase7SummaryWorth">{row.startWorth ?? "-"}</span>
              <SummarySlots guids={row.traitGuids} count={4} kind="trait" />
              <SummarySlots guids={row.modGuids} count={6} kind="mod" />
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function SummarySlots({ guids, count, kind }: { guids: string[]; count: number; kind: "trait" | "mod" }) {
  return (
    <div className={`phase7SummarySlots ${kind}`} aria-label={kind === "trait" ? "异能" : "装备"}>
      {Array.from({ length: count }, (_, index) => {
        const guid = guids[index];
        const info = resolveArenaModTraitInfo(guid);
        const label = info?.name ?? guid;
        return (
          <span className={kind === "trait" ? "phase7SummaryTraitSlot" : "phase7SummaryModSlot"} key={`${kind}-${index}`}>
            {guid && info?.iconUrl ? <img src={info.iconUrl} alt={label} /> : guid ? <span>{guid}</span> : null}
          </span>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 4: 运行弹窗与计算测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
```

Expected: PASS。

- [ ] **Step 5: 提交弹窗单元**

```powershell
git add apps/owstats-desktop/src/features/matches/ArenaPlayerRoundSummaryDialog.tsx apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx
git commit -m "feat: add stadium round summary dialog"
```

### Task 3: 接入角斗汇总英雄头像

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Modify: `apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx`
- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`

- [ ] **Step 1: 添加入口契约失败测试**

在 `arenaPlayerRoundSummary.test.tsx` 读取两个组件源码并断言：

```tsx
import { readFileSync } from "node:fs";

const compactSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const tableSource = readFileSync(new URL("../src/features/matches/MatchTeamTable.tsx", import.meta.url), "utf8");

assert.match(compactSource, /selected\.key === "summary" \? openArenaPlayerSummary : undefined/);
assert.match(compactSource, /buildArenaPlayerRoundSummary\(summaryTarget\.player, arena\.rounds\)/);
assert.match(tableSource, /onSelectPlayerSummary\?: \(player: PlayerRow, trigger: HTMLButtonElement\) => void/);
assert.match(tableSource, /aria-label=\{`查看\$\{player\.name\}的汇总卡片`\}/);
```

同时将 `arenaDetailFormat.test.ts` 中要求整个可展开行自身是按钮、直接持有 `onClick={onToggle}` 和 `aria-expanded` 的旧断言，更新为要求：

```ts
assert.equal(teamTableSource.includes("phase7CompactPlayerRowToggle"), true);
assert.equal(teamTableSource.includes("phase7CompactAvatarSummaryButton"), true);
assert.equal(teamTableSource.includes("onSelectPlayerSummary"), true);
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
npx tsx scripts/arenaDetailFormat.test.ts
```

Expected: 两项均 FAIL，分别缺少新入口和新行结构。

- [ ] **Step 3: 扩展 `MatchTeamTable` 的可选头像回调**

新增 prop：

```ts
onSelectPlayerSummary?: (player: PlayerRow, trigger: HTMLButtonElement) => void;
```

将玩家行内容拆成头像与其余内容。可展开行使用一个非交互布局容器，内部放两个并列按钮，禁止按钮嵌套：

```tsx
<div className="phase6CompactPlayerRow phase7CompactPlayerExpandable" aria-expanded={expanded}>
  <button
    type="button"
    className="phase6CompactAvatarPlate phase7CompactAvatarSummaryButton"
    onClick={(event) => onSelectPlayerSummary?.(player, event.currentTarget)}
    aria-label={`查看${player.name}的汇总卡片`}
  >
    <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
  </button>
  <button
    type="button"
    className="phase7CompactPlayerRowToggle"
    onClick={onToggle}
    aria-expanded={expanded}
    aria-label={`${player.name}${expanded ? "收起角斗详情" : "展开角斗详情"}`}
  >
    {contentWithoutAvatar}
  </button>
</div>
```

当 `onSelectPlayerSummary` 未提供时，头像保持非按钮展示；普通对局不得获得新操作。

- [ ] **Step 4: 在 `CompactArenaDetail` 持有弹窗状态**

增加状态与稳定关闭函数：

```tsx
const [summaryTarget, setSummaryTarget] = useState<{
  player: PlayerRow;
  trigger: HTMLButtonElement;
} | null>(null);

function openArenaPlayerSummary(player: PlayerRow, trigger: HTMLButtonElement) {
  setSummaryTarget({ player, trigger });
}

function closeArenaPlayerSummary() {
  const trigger = summaryTarget?.trigger;
  setSummaryTarget(null);
  requestAnimationFrame(() => trigger?.focus());
}
```

匹配切换和回合切换时同步清空弹窗目标。两个 `MatchTeamTable` 均只在汇总视图传入：

```tsx
onSelectPlayerSummary={selected.key === "summary" ? openArenaPlayerSummary : undefined}
```

在角斗详情根节点末尾渲染：

```tsx
{summaryTarget && (
  <ArenaPlayerRoundSummaryDialog
    player={summaryTarget.player}
    rows={buildArenaPlayerRoundSummary(summaryTarget.player, arena.rounds)}
    onClose={closeArenaPlayerSummary}
  />
)}
```

- [ ] **Step 5: 运行针对性测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
npx tsx scripts/arenaDetailFormat.test.ts
npx tsx scripts/matchWorkbench.test.ts
```

Expected: PASS。若 `matchWorkbench.test.ts` 中存在旧的整行按钮源码契约，只将其更新为并列按钮结构，不放宽普通详情和角斗展开行为断言。

- [ ] **Step 6: 提交交互接入**

```powershell
git add apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx apps/owstats-desktop/scripts/arenaDetailFormat.test.ts apps/owstats-desktop/scripts/matchWorkbench.test.ts
git commit -m "feat: open stadium summaries from hero avatars"
```

### Task 4: 完成视觉、响应式与测试注册

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: 添加视觉契约失败测试**

在新测试中读取 `matches.css`，断言：

```tsx
const styles = readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8");
assert.match(styles, /\.phase7SummaryDialogBackdrop\s*\{[^}]*position:\s*fixed/s);
assert.match(styles, /\.phase7SummaryDialog\s*\{[^}]*max-height:\s*min\(760px,\s*calc\(100dvh - 48px\)\)/s);
assert.match(styles, /\.phase7SummaryRound\s*\{[^}]*grid-template-columns:/s);
assert.match(styles, /\.phase7SummarySlots\.trait\s*\{[^}]*grid-template-columns:\s*repeat\(4,/s);
assert.match(styles, /\.phase7SummarySlots\.mod\s*\{[^}]*grid-template-columns:\s*repeat\(6,/s);
assert.match(styles, /@media\s*\(max-width:\s*900px\)/s);
assert.doesNotMatch(styles, /\.phase7SummaryDialog[\s\S]*overflow-x:\s*(auto|scroll)/s);
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
```

Expected: FAIL，提示缺少弹窗样式。

- [ ] **Step 3: 添加符合 `design.md` 的样式**

在 `matches.css` 增加：

```css
.phase7CompactPlayerExpandable {
  grid-template-columns: 86px minmax(0, 1fr);
  gap: 0;
  padding: 0;
}

.phase7CompactAvatarSummaryButton {
  width: 100%;
  border: 0;
  cursor: pointer;
}

.phase7CompactAvatarSummaryButton:focus-visible,
.phase7CompactPlayerRowToggle:focus-visible {
  outline: 2px solid var(--data-primary);
  outline-offset: 2px;
}

.phase7CompactPlayerRowToggle {
  min-width: 0;
  min-height: 66px;
  display: grid;
  grid-template-columns:
    minmax(150px, 1fr) minmax(72px, 0.3fr) minmax(170px, 0.64fr)
    minmax(122px, 0.45fr) minmax(96px, 0.34fr);
  align-items: center;
  gap: 12px;
  padding: 0 14px 0 12px;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.phase7SummaryDialogBackdrop {
  position: fixed;
  inset: 0;
  z-index: 120;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(32 34 32 / 0.36);
}

.phase7SummaryDialog {
  width: min(1040px, calc(100vw - 48px));
  max-height: min(760px, calc(100dvh - 48px));
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-primary);
  background: color-mix(in srgb, var(--glass-primary) 92%, var(--app-canvas));
  box-shadow: 0 10px 28px rgba(45, 54, 48, 0.045);
}

.phase7SummaryDialog > header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 12px;
  padding: 20px 24px;
  border-bottom: 1px solid var(--glass-border);
}

.phase7SummaryDialog h2 {
  margin: 0;
  color: var(--text-primary);
  font-size: 20px;
  font-weight: 400;
}

.phase7SummaryDialogBody {
  min-height: 0;
  padding: 8px 24px 24px;
  overflow-y: auto;
}

.phase7SummaryRound {
  min-height: 72px;
  display: grid;
  grid-template-columns: 44px 92px minmax(220px, 0.8fr) minmax(300px, 1.2fr);
  align-items: center;
  gap: 16px;
  border-bottom: 1px solid var(--glass-border);
}

.phase7SummaryWorth {
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.phase7SummarySlots {
  display: grid;
  gap: 8px;
}

.phase7SummarySlots.trait {
  grid-template-columns: repeat(4, minmax(40px, 1fr));
}

.phase7SummarySlots.mod {
  grid-template-columns: repeat(6, 44px);
}

.phase7SummaryTraitSlot,
.phase7SummaryModSlot {
  min-width: 0;
  height: 44px;
  display: grid;
  place-items: center;
  border: 1px solid var(--glass-border);
  background: rgba(250, 251, 247, 0.38);
  overflow: hidden;
}

.phase7SummaryTraitSlot img,
.phase7SummaryModSlot img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

@media (max-width: 900px) {
  .phase7SummaryDialog {
    width: calc(100vw - 24px);
    max-height: calc(100dvh - 24px);
  }

  .phase7SummaryRound {
    grid-template-columns: 40px 72px minmax(160px, 0.8fr) minmax(240px, 1.2fr);
    gap: 8px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .phase7SummaryDialogBackdrop,
  .phase7SummaryDialog {
    transition: none;
  }
}
```

如现有变量名与示例不一致，只能替换为 `design.md` 已定义的同语义变量，不得添加新品牌颜色。

- [ ] **Step 4: 注册测试**

在 `package.json` 的 `test:components` 命令中，在 `matchWorkbenchVisualFixture.test.ts` 前加入：

```json
"tsx scripts/arenaPlayerRoundSummary.test.tsx"
```

- [ ] **Step 5: 运行针对性测试和构建**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
npx tsx scripts/arenaDetailFormat.test.ts
npx tsx scripts/matchWorkbench.test.ts
npm run build
```

Expected: 所有命令 PASS；构建生成 Vite 产物且 `verifyDistPaths` 通过。

- [ ] **Step 6: 运行完整前端验证**

Run:

```powershell
cd apps\owstats-desktop
npm test
```

Expected: `test:resources`、`test:unit`、`test:components`、`test:legacy`、`test:text` 全部 PASS。

- [ ] **Step 7: 检查差异与隐私边界**

Run:

```powershell
cd ..\..
git diff --check
git status --short
git diff -- apps/owstats-desktop/src/features/matches apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx apps/owstats-desktop/package.json
```

Expected:

- `git diff --check` 无输出。
- 本功能没有后端、SQLite、同步或 DTO 改动。
- 测试只包含 `synthetic-player` 等合成值。
- 工作树中开始任务前已有的用户改动仍保持原状，未被覆盖或暂存。

- [ ] **Step 8: 提交视觉与验证**

```powershell
git add apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx apps/owstats-desktop/package.json
git commit -m "feat: style stadium player summary cards"
```

### Task 5: 最终人工验收

**Files:**
- Verify only; no planned file changes.

- [ ] **Step 1: 使用匿名 fixture 启动应用**

Run:

```powershell
cd apps\owstats-desktop
npm run tauri:dev
```

Expected: 应用正常打开，且调试输出不包含 ID、路径、payload 或凭据。

- [ ] **Step 2: 验收角斗汇总**

使用安全本地 fixture 打开一场有多个回合的角斗对局，确认：

- 角斗汇总中的队友和对手英雄头像均可点击。
- 单击头像只打开汇总卡片，不展开玩家行。
- 标题栏显示“汇总卡片”、英雄头像和玩家名称。
- R1 资金为 3000，R2 使用 R1 累计资金，后续依次错位。
- R1 一个异能、R2 两个异能等成长过程与回合结束快照一致。
- 每行固定四个异能槽、六个装备槽。
- 图标没有名称、说明或悬停提示。
- 右上角、遮罩、Esc 均可关闭，焦点回到原头像。

- [ ] **Step 3: 验收回归与窗口尺寸**

确认：

- 角斗单回合头像不打开汇总卡片。
- 角斗玩家行原有展开仍可使用。
- 普通对局行为不变。
- 在 `1920×1080` 与 `1440×900` 下弹窗不产生横向滚动；内容过高时只滚动弹窗内容区。

- [ ] **Step 4: 最终状态检查**

Run:

```powershell
git diff --check
git status --short
```

Expected: 无空白错误；只出现本功能预期文件和任务开始前已存在的用户改动。
