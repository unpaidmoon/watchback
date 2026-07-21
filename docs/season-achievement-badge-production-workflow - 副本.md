# 赛季成就徽章生产流程

本文固化“赛季成就墙”后续新增成就徽章的生产方法。目标是保证未来新增成就时，徽章与当前 `token-style` 资产保持同一套视觉语言、命名规则和验收标准。

## 适用范围

本流程适用于 `assets/achievements/token-style` 下的成就徽章资产。

每个成就固定产出三枚等级徽章：

- `I`：基础等级，结构清楚，金属质感克制。
- `II`：进阶等级，边框、材质和光效增强。
- `III`：最高等级，仪式感最强，外框、底座和高光最完整。

每次只处理一组成就，不再一次性生成大量徽章合集。

## 风格基准

当前定稿风格为 MOBA challenge token 式收藏徽章，接近英雄联盟 Challenges token 的视觉方向，但不使用任何官方 Logo、英雄、武器或可识别版权元素。

固定要求：

- 形态：圆形、盾形或 token medallion，中心有抽象符号，外围有金属边框。
- 构图：一组成就横向三枚徽章，从左到右对应 `I / II / III`。
- 递进：三个等级必须看出材质、边框复杂度、亮度和仪式感逐级提高。
- 背景：透明或深色中性背景，方便裁切和接入深色 UI。
- 识别：缩小到卡片尺寸后，中心符号仍然能识别。

禁止项：

- 不要中文、英文、数字或罗马数字出现在图片内。
- 不要游戏 Logo、战队标志、英雄脸、具体武器或游戏截图。
- 不要未来赛博、强霓虹、科幻 HUD、过度发光的界面元素。
- 不要一次生成 24 宫格或多组成就混合图。
- 不要为未激活状态单独生成暗色图片；未激活效果由 CSS 处理。

## 资产命名

新增成就使用稳定的英文 key。文件命名必须与现有规则一致：

```text
assets/achievements/token-style/{achievement-key}-token-group.png
assets/achievements/token-style/{achievement-key}-token-preview.png
assets/achievements/token-style/{achievement-key}-token-tier-1.png
assets/achievements/token-style/{achievement-key}-token-tier-2.png
assets/achievements/token-style/{achievement-key}-token-tier-3.png
```

说明：

- `group`：Image2 原始三联图或清理后的三联图。
- `preview`：给用户检查的预览图，可包含轻微留白或统一展示背景。
- `tier-1/2/3`：实际前端加载的单枚徽章。

## 新增成就输入模板

新增成就前，先补齐下面信息：

```text
成就 key：
中文名：
成就含义：
等级 I 条件：
等级 II 条件：
等级 III 条件：
核心视觉语义：
主符号建议：
颜色倾向：
明确避免：
```

示例：

```text
成就 key：comeback
中文名：逆风翻盘
成就含义：落后局势下完成胜利
等级 I 条件：1 次逆风胜利
等级 II 条件：3 次逆风胜利
等级 III 条件：8 次逆风胜利
核心视觉语义：落后、反击、翻盘、逆转
主符号建议：向上翻转的箭头、破碎后重组的盾、反向旋转的星环
颜色倾向：青铜暗金、银蓝、金白
明确避免：比分面板、具体英雄、武器、文字
```

## Image2 Prompt 模板

每次生成一组成就时，使用下面模板，并替换方括号内容。

```text
Create a horizontal set of three fantasy challenge token achievement badges for a competitive game UI.

Achievement theme: [English description of the achievement theme]
Visual symbol: [main abstract symbol, no text]

Style reference: elegant MOBA challenge token badges, ornate metallic frame, collectible rank medallions, consistent shape language, premium game achievement UI.

Three tiers from left to right:
Tier 1 is restrained bronze with a clear simple frame.
Tier 2 is refined silver with stronger material detail and subtle cool highlights.
Tier 3 is prestigious gold with richer ornament, stronger glow, and the most ceremonial frame.

No text, no letters, no numbers, no logos, no characters, no weapons, no screenshots.
Clean centered icons, transparent or dark neutral background, consistent lighting, high detail, suitable for cropping into three separate square badge icons.
```

补充规则：

- Prompt 必须明确 “horizontal set of three”。
- Prompt 必须明确 “No text, no letters, no numbers”。
- Prompt 必须描述主符号，而不是只写中文成就名。
- 如果第一次生成偏离风格，只微调符号、材质和光效，不改变整体 token-style 方向。

## 生产步骤

1. 定义成就信息

   先确认 key、中文名、含义、三级条件。key 一旦落库就不要随意改名。

2. 提炼视觉语义

   把成就玩法翻译成抽象符号。优先使用箭头、盾、星环、冠冕、火焰、碎片、棱镜、轨迹等通用符号，避免具体游戏物件。

3. 生成三联图

   使用 Image2 一次只生成当前这一组成就的三枚等级徽章。

4. 初审三联图

   检查是否符合风格基准，重点看三件事：无文字、同系列、等级递进清楚。

5. 用户确认

   把预览图给用户检查。用户满意后再裁切；不满意则继续调整 Prompt 重新生成。

6. 裁切单枚徽章

   将三联图裁成三个居中的单枚 PNG。主体不能贴边、断边或大小明显不一致。

7. 落库命名

   按命名规则保存到 `assets/achievements/token-style`。

8. 接入前端

   在成就配置中使用同一个 achievement key，使前端可以通过 `achievementBadgeUrl(key, level)` 加载对应等级图片。

9. 回归检查

   检查卡片态、详情弹窗态、未激活暗处理态。未激活徽章应使用同一张图片配合 CSS 灰暗处理。

## 验收清单

新增徽章落库前必须逐项确认：

- 三枚徽章属于同一组视觉体系。
- `I / II / III` 的等级递进清楚。
- 和现有 8 组成就一起展示时不突兀。
- 图片内没有文字、字母、数字、Logo、英雄、武器、截图。
- 中心符号在小尺寸卡片中仍然可辨认。
- 裁切后主体居中，边缘完整，三枚尺寸一致。
- 未激活 CSS 暗处理后仍能看出轮廓。
- 最高等级有收藏感，但没有过度赛博或强霓虹。

## 使用方式

以后新增成就时，直接按下面格式发给 Codex：

```text
按照 docs/season-achievement-badge-production-workflow.md 新增一组成就徽章。

成就 key：
中文名：
成就含义：
等级 I 条件：
等级 II 条件：
等级 III 条件：
核心视觉语义：
主符号建议：
颜色倾向：
明确避免：

先生成三联预览图给我检查，不要裁切，不要接入前端。
```

如果只想先探索风格，可以把最后一句改成：

```text
先给我 2-3 个主符号方向，不生成图片。
```

如果已经确认预览图满意，再继续发：

```text
这组成就徽章满意，按流程裁切成 tier-1/2/3，保存到 assets/achievements/token-style，并接入成就配置。
```
