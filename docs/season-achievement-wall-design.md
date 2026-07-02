# 赛季成就墙产品设计

本文档定义“战报 / 赛季成就墙”的第一版产品方案。目标是把当前偏数据汇总的成就卡片，升级为奖杯陈列柜式的三级成就徽章墙。本文只写产品方案，不作为完整开发规格。

## 设计目标

- 让用户进入战报页时，第一眼看到本赛季获得的奖章和荣誉。
- 让成就墙更像奖杯陈列柜，而不是任务列表或数据表。
- 每个成就固定分为 I / II / III 三级，强调视觉进阶。
- 点击成就卡片后，用居中弹窗展示三级徽章详情。
- 徽章资产使用 Image2 生成，整体风格统一，靠近守望先锋竞技 UI 的未来金属感，但不使用官方 Logo、英雄元素或可识别版权图形。

## 非目标

- 第一版不做截图分享按钮。
- 第一版不强调证据对局入口。
- 第一版不重做成独立视觉场景，继续沿用 Phase 6 的玻璃面板页面风格。
- 第一版不扩展到完整成就系统，只重做现有 8 个成就。
- 第一版不为未激活徽章单独生成暗色图片，未激活状态由 CSS 处理。

## 用户体验定位

成就墙主要服务“社交炫耀”和“奖杯展示”感受。用户不需要先读规则，就能通过徽章尺寸、金属框、等级和暗亮状态判断自己本赛季达成了什么。

页面首屏应更像奖杯陈列柜：

- 徽章和称号是主视觉。
- 数据只作为辅助说明。
- 首页保持干净，避免强任务感。
- 三级等级信息可以探索获得，不在默认状态塞满说明。

## 信息架构

战报页保留现有结构，赛季成就墙作为其中一个玻璃面板模块。

成就墙首页包含：

- 标题：赛季成就墙。
- 简短说明：把胜率、高光、英雄池和连胜记录转成赛季奖章。
- 8 张成就卡片。
- 每张卡片展示当前激活等级的主徽章。
- 首页进度条低调展示，只使用细线，不抢主徽章视觉。
- I / II / III 等级槽只在 hover 时显示，保持首页干净。

点击任意成就卡片后打开居中弹窗。

弹窗包含：

- 成就标题。
- 当前等级，例如 `II`。
- 三枚等级徽章，桌面端固定横排展示。
- 当前激活徽章使用金属框升级效果突出。
- 未激活徽章保留完整图形，但灰暗、低饱和。
- 下方展示当前数值、下一等级门槛和进度条。

## 成就范围

第一版保留现有 8 个成就方向，只做三级和 UI 重设计。

| Key | 成就名 | 含义 |
| --- | --- | --- |
| `win-streak` | 常胜将军 | 最长连胜 |
| `win-rate` | 胜率机器 | 赛季胜率 |
| `s-grade` | 高光制造者 | S 级表现次数 |
| `carry` | 战局核心 | 带飞局次数 |
| `signature` | 招牌打法 | 最常出现的个人高光标签 |
| `peak` | 巅峰一战 | 单场最佳表现 |
| `hero-focus` | 本命专精 | 第一英雄占比 |
| `switching` | 战术变阵 | 英雄切换和英雄池宽度 |

## 三级等级

每个成就固定有三个等级，命名统一使用 `I / II / III`。不使用铜银金，也不为每个成就写专属等级名。

三级之间主要体现视觉进阶：

- I：基础徽章，结构清楚，金属质感克制。
- II：边框和材质升级，加入更明显的蓝橙高对比或冷暖层次。
- III：最高等级，金属底座、外框和高光最完整，仪式感最强。

规则仍需要存在，保证等级有含义；但视觉优先于门槛解释。

示例门槛建议：

| 成就 | I | II | III |
| --- | ---: | ---: | ---: |
| 常胜将军 | 3 连胜 | 5 连胜 | 8 连胜 |
| 胜率机器 | 50% | 55% | 60% |
| 高光制造者 | 5 场 S | 15 场 S | 30 场 S |
| 战局核心 | 5 场带飞 | 12 场带飞 | 25 场带飞 |
| 招牌打法 | 出现 3 次 | 出现 8 次 | 出现 15 次 |
| 巅峰一战 | 有单场峰值 | 峰值进入高门槛 | 峰值进入极高门槛 |
| 本命专精 | 第一英雄占比 25% | 40% | 55% |
| 战术变阵 | 使用 5 名英雄 | 10 名英雄 | 15 名英雄 |

具体阈值后续实现时可以按真实赛季样本微调，但第一版产品表达固定为三级。

## 首页卡片

首页卡片应从“数据卡”改成“徽章卡”。

卡片默认结构：

- 左侧或上方：当前激活等级徽章。
- 标题：成就名。
- 副文案：一句荣誉描述。
- 当前值：短文本，例如 `最长连胜 7 局`。
- 低调细进度线：显示到下一等级的进度。

卡片 hover：

- 显示 I / II / III 三个小等级槽。
- 当前等级槽高亮。
- 未激活等级槽暗处理。
- 徽章轻微浮起或金属反光增强。

卡片点击：

- 打开居中弹窗。
- 不跳页。
- 不在首页直接展开详情。

## 详情弹窗

详情弹窗负责解释和展示，但视觉仍优先。

弹窗布局：

1. 顶部：成就名、当前等级、关闭按钮。
2. 中部：三枚徽章固定横排。
3. 底部：当前数值、下一等级门槛、低调进度条。

三枚徽章状态：

- 当前激活：金属框升级，底座更完整，边框更亮。
- 已解锁但非当前：正常亮度，但弱于当前。
- 未激活：同一张原图 CSS 暗处理，低饱和、低亮度，不加锁，不做剪影。

桌面端三个徽章固定横排展示。第一版暂不考虑移动端，Tauri 桌面优先。

## 视觉原则

整体继续沿用 Phase 6 的玻璃面板风格：

- 页面框架不重做。
- 成就墙区域保持玻璃面板、暗色背景、克制边框。
- 主要视觉升级集中在徽章、卡片和弹窗。

徽章风格：

- 守望先锋竞技感。
- 未来科技。
- 抛光金属。
- 清晰轮廓。
- 蓝橙高对比。
- 等级越高，外框、底座、材质和高光越复杂。

避免：

- 官方 Overwatch Logo。
- 具体英雄外观或武器。
- 过度手游化的大量宝石。
- 文字、数字、可读字母。
- 复杂背景。
- 水印。

## 徽章资产

第一版每个成就生成 3 张独立徽章，共 24 张。

命名建议：

```text
apps/owstats-desktop/src/assets/achievements/win-streak-level-1.png
apps/owstats-desktop/src/assets/achievements/win-streak-level-2.png
apps/owstats-desktop/src/assets/achievements/win-streak-level-3.png
```

徽章图片不要求透明背景。每张徽章自带固定深色底座或圆形背景，以保证风格统一、边缘稳定、展示效果可控。

首页和弹窗共用同一套资产。未激活状态通过 CSS 处理：

- `filter: grayscale(1) saturate(0.35) brightness(0.55);`
- `opacity: 0.45;`
- 可加一层暗色遮罩，但不遮挡主体轮廓。

## Image2 生成提示词

每个成就每个等级单独生成一张。生成时保持同一构图、同一相机角度、同一底座语言。

通用提示词模板：

```text
Use case: stylized-concept
Asset type: game achievement badge icon for a desktop Overwatch statistics app
Primary request: Create one premium competitive achievement badge for [ACHIEVEMENT_NAME], level [I/II/III].
Subject: A centered [SUBJECT_SYMBOL] emblem that represents [ACHIEVEMENT_MEANING].
Style/medium: futuristic competitive game UI badge, polished metal, beveled rim, enamel inlay, readable silhouette, Overwatch-inspired esports energy without using any official logo or hero design.
Composition/framing: square icon, centered badge, fixed dark circular or shield-like base, generous padding, front-facing view.
Lighting/mood: crisp studio UI lighting, controlled highlights, premium reward feel.
Color palette: blue and orange high contrast accents, dark gunmetal base; level I restrained metal, level II stronger frame and glow, level III most ornate metal frame and heroic highlight.
Materials/textures: brushed metal, hard-surface bevels, subtle glass enamel, clean rim light.
Text: no text.
Constraints: no letters, no numbers, no readable text, no watermark, no official Overwatch logo, no specific Overwatch hero, no weapon copied from the game.
Avoid: cluttered background, photorealistic trophy photo, fantasy medieval style, excessive gems, flat vector icon.
```

成就主题符号建议：

| 成就 | Subject Symbol |
| --- | --- |
| 常胜将军 | crown with forward chevrons |
| 胜率机器 | precision gauge or tactical dial |
| 高光制造者 | spark burst or radiant star core |
| 战局核心 | crossed energy blades or command core |
| 招牌打法 | target reticle with signature mark |
| 巅峰一战 | trophy peak with upward beam |
| 本命专精 | shield with hero-focus silhouette |
| 战术变阵 | rotating arrows around modular plates |

## 文案原则

首页文案短，避免解释过多。

示例：

- 常胜将军：连续胜利奖章。
- 胜率机器：赛季稳定输出。
- 高光制造者：S 级表现收藏。
- 战局核心：关键胜局推动者。
- 招牌打法：最常出现的个人高光。
- 巅峰一战：本赛季单场峰值。
- 本命专精：主力英雄熟练度。
- 战术变阵：英雄池变化能力。

详情文案可以补充门槛：

```text
当前：最长连胜 7 局
下一等级：III，需要 8 局
```

## 验收标准

- 用户在战报页首屏能明确看到“赛季成就墙”是奖杯墙，而不是普通数据卡。
- 每张成就卡片都能看出当前激活等级。
- 点击卡片后能看到 I / II / III 三枚徽章。
- 当前等级通过金属框升级高亮，而不是主要依靠光晕或放大。
- 未激活徽章完整可见，但明显暗处理。
- 首页进度线低调，不压过徽章。
- 8 个成就全部保留。
- 24 张徽章风格一致，没有官方 Logo、英雄元素、文字或水印。

## 后续可选扩展

- 增加截图分享布局。
- 增加英雄专属成就。
- 增加好友组排成就。
- 增加赛季主称号和三枚代表徽章。
- 移动端详情弹窗改成横向滑动徽章。
