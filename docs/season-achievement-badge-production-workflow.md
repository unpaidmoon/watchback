# 赛季成就徽章生产流程

本文固化“赛季成就墙”后续新增成就徽章的生产方法。目标是让新徽章与当前 `token-style` 资源保持同一套视觉语言、等级递进、开放式透明结构、命名规则和验收标准。

## 适用范围

本流程适用于 `assets/achievements/token-style` 下的成就徽章资源。

每个成就固定产出三枚等级徽章：

- `I`：基础等级，结构清楚，金属质感克制。
- `II`：进阶等级，边框、材质和细节增强，采用银蓝色层级。
- `III`：最高等级，仪式感最强，采用金色层级和更丰富的装饰。

每次只处理一组成就。三联图只用于确认设计方向，正式资源必须逐级单独生成和处理，不能直接从三联预览图裁切。

## 风格基准

当前定稿风格为 MOBA challenge token 式收藏徽章，接近英雄联盟 Challenges token 的视觉方向，但不得使用任何官方 Logo、英雄、武器或可识别版权元素。

固定要求：

- 形态：圆形、盾形或 token medallion，中心有抽象符号，外围有金属边框。
- 构图：三档等级共享同一核心符号和形态语言。
- 递进：`I / II / III` 的材质、边框复杂度和仪式感逐级提高。
- 配色：`I` 使用克制的青铜色，`II` 使用银蓝色，`III` 使用金色。
- 识别：缩小到成就墙卡片尺寸后，中心符号仍然清楚可辨。
- 光效：正式徽章只保留附着于实体材质上的高光；背景辉光、光晕、放射线和粒子由前端动态光效独立渲染。

禁止项：

- 不要中文、英文、数字或罗马数字出现在图片内。
- 不要游戏 Logo、战队标志、英雄脸、具体武器或游戏截图。
- 不要未来赛博、强霓虹、科幻 HUD 或过度发光的界面元素。
- 不要一次生成 24 宫格或多组成就混合图。
- 不要为未激活状态单独生成暗色图片；未解锁效果由 CSS 处理。
- 不要把深色圆盘、矩形卡片、投影或背景光晕烘焙进正式图片。

## 开放式透明底规范

“透明底”不仅表示画布四角透明，还要求徽章内部的开放区域真正透出页面背景。

- 外圈、枝叶、尖角、桁架和中心符号之间的空隙必须透明。
- 不得用黑色、深灰色或半透明圆盘填充徽章内部空隙。
- 不得保留矩形背景、渐变背景、纹理背景、地面投影或外发光。
- 金属框、主体符号、冠饰、宝石以及附着在实体零件上的珐琅色和材质反光必须保留。
- 主体轮廓要有清楚的硬边缘，避免透明化后出现洋红残边、绿色污染或灰黑色毛边。
- 正式资源统一为 `512×512` RGBA PNG，主体等比缩放到不超过 `480×480`，在透明画布内居中。

开放式结构的判断方法：把徽章同时放在深色和浅色背景上。如果圆环或枝叶之间仍出现一整块固定深色底，说明资源不合格，需要重新生成，不能用调色或遮盖掩盖。

## 资源命名

新增成就使用稳定的英文 key。文件命名必须与现有规则一致：

```text
assets/achievements/token-style/{achievement-key}-token-group.png
assets/achievements/token-style/{achievement-key}-token-preview.png
assets/achievements/token-style/{achievement-key}-token-tier-1.png
assets/achievements/token-style/{achievement-key}-token-tier-2.png
assets/achievements/token-style/{achievement-key}-token-tier-3.png
```

说明：

- `group`：用于评审整体设计方向的三联图，不作为正式资源输入。
- `preview`：给用户检查的展示图，可以使用统一的深色预览背景。
- `tier-1/2/3`：前端实际加载的透明底单枚徽章。

透明化处理中间文件统一放在临时目录，不进入前端资源引用：

```text
tmp/achievement-transparent/{achievement-key}/tier-1-key.png
tmp/achievement-transparent/{achievement-key}/tier-2-key.png
tmp/achievement-transparent/{achievement-key}/tier-3-key.png
```

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
等级 I 条件：3 次逆风胜利
等级 II 条件：10 次逆风胜利
等级 III 条件：30 次逆风胜利
核心视觉语义：落后、反击、翻盘、逆转
主符号建议：向上翻转的箭头、破碎后重组的盾、反向旋转的星环
颜色倾向：青铜暗金、银蓝、金白
明确避免：比分面板、具体英雄、武器、文字
```

## 设计预览 Prompt 模板

设计阶段可以先生成横向三联图，以便确认三档徽章是否属于同一系列。三联图可以使用统一的深色展示背景，但不能直接作为正式透明资源。

```text
Create a horizontal set of three fantasy challenge token achievement badges for a competitive game UI.

Achievement theme: [English description of the achievement theme]
Visual symbol: [main abstract symbol, no text]

Style reference: elegant MOBA challenge token badges, ornate metallic frames, collectible rank medallions, consistent shape language, premium game achievement UI.

Three tiers from left to right:
Tier 1 is restrained bronze with a clear simple frame.
Tier 2 is refined silver-blue with stronger material detail.
Tier 3 is prestigious gold with richer ornament and the most ceremonial frame.

Use an open-frame construction with visible gaps between rings, leaves, ornaments, and the central symbol. Do not place a solid dark disc behind the badge.
No text, no letters, no numbers, no logos, no characters, no weapons, no screenshots.
Centered badges, consistent scale and lighting, high detail. This image is a design preview only.
```

## 正式单枚资源 Prompt 模板

三联设计确认后，`I / II / III` 必须分别以对应徽章为参考单独生成。正式资源使用固定的纯洋红色键背景，禁止使用“透明或深色背景”这类不确定描述。

```text
Create one centered [bronze / silver-blue / gold] achievement token badge matching the approved design reference.

Keep the same core symbol, frame language, proportions, gemstones, and tier identity as the reference. Use an open-frame construction: all gaps between rings, branches, leaves, spikes, ornaments, and the central symbol must expose the background directly.

Place the badge on a completely uniform solid #ff00ff chroma-key background. The background must contain no gradient, texture, shadow, reflection, vignette, glow, rays, particles, or dark disc. Keep only highlights that belong to the physical metal, enamel, gemstone, or solid badge parts.

One front-facing badge only, centered with even empty space around it. Crisp hard silhouette, no text, no letters, no numbers, no logo, no watermark, no additional symbols or decorations. Do not fill the open interior regions with black or dark color.
```

补充规则：

- 每个等级单独生成，不从三联图裁切正式资源。
- 色键背景必须是完全均匀的 `#ff00ff`，不能出现暗角、阴影或渐变。
- 如果内部仍有暗色底盘，重新生成，并明确指出哪些圆环、叶片或结构之间必须露出洋红背景。
- 如果透明化后金色偏绿或出现洋红残边，重新生成无外发光、硬边缘版本，不接受通过调色掩盖污染。

## 生产步骤

1. **定义成就信息**

   确认 key、中文名、含义和三级条件。key 一旦落库，不要随意改名。

2. **提炼视觉语义**

   把成就玩法转译成抽象符号。优先使用箭头、盾、星环、冠冕、火焰、碎片、棱镜、轨迹等通用符号，避免具体游戏物件。

3. **生成三联设计预览**

   一次只生成当前这一组成就的三档设计，用于检查系列一致性和等级递进。

4. **初审三联图**

   检查无文字、同系列、等级递进清楚，并确认构图具备开放式结构，没有必须依赖暗色底盘才能成立的部分。

5. **用户确认设计**

   把预览图交给用户检查。满意后才进入正式资源制作；不满意则调整符号、材质或结构后重新生成。

6. **逐级生成色键图**

   分别生成 `tier-1-key.png`、`tier-2-key.png` 和 `tier-3-key.png`。每张图只包含一枚正面居中的徽章，背景固定为纯 `#ff00ff`。

7. **检查开放式结构**

   确认徽章外部及内部空隙都直接露出色键背景；确认没有矩形底、暗色圆盘、投影、背景光晕、放射线或粒子。

8. **转换透明 PNG**

   在仓库根目录运行：

   ```powershell
   python scripts/process_achievement_token_transparency.py `
     --group <achievement-key> `
     --input-dir tmp/achievement-transparent/<achievement-key>
   ```

   脚本会执行边界自动取色、软遮罩、去色键污染和 Alpha 校验，然后把主体等比缩放至不超过 `480×480`，居中写入透明的 `512×512` 画布，并输出到 `assets/achievements/token-style/<achievement-key>-token-tier-N.png`。

9. **制作双背景预览**

   在深色和浅色背景上并排检查三档正式 PNG。预览背景只用于验收，不能合并进正式资源。

10. **接入前端**

    在成就配置中使用同一个 achievement key，通过 `achievementBadgeUrl(key, level)` 加载对应等级图片。背景动态光效继续使用独立的二级紫蓝、三级金色资源，不烘焙进徽章 PNG。

11. **回归检查**

    检查成就墙、成就详情、未解锁暗处理、深浅背景边缘以及动态光效叠加。未解锁徽章使用同一张图片配合 CSS 灰暗处理。

## 验收清单

新增徽章落库前必须逐项确认：

- 三枚徽章属于同一组视觉体系，`I / II / III` 等级递进清楚。
- 三枚正式资源均为 `512×512` RGBA PNG。
- Alpha 通道同时包含完全透明和完全不透明像素，四角 Alpha 为 `0`。
- 徽章外部和内部开放区域都是真正透明，没有矩形底或暗色圆盘。
- 深色与浅色背景上均无洋红描边、绿色污染、灰黑毛边或异常偏色。
- 图片内没有文字、字母、数字、Logo、英雄、武器或截图。
- 中心符号在成就墙卡片尺寸下仍可辨认。
- 主体居中、边缘完整，三档视觉尺寸一致，没有贴边或断边。
- 正式 PNG 不含背景投影、外发光、放射线或粒子；动态光效由前端单独显示。
- 未解锁 CSS 暗处理后仍能看出徽章轮廓。
- 最高等级有收藏感，但不过度赛博或强霓虹。

## 验证命令

资源接入后运行：

```powershell
cd apps/owstats-desktop
npm run prepare:assets
npm run test:resources
npm run build
cd ../..
git diff --check
```

如果只更新本流程文档，不需要重新构建前端；仍需运行 `git diff --check` 并检查文档差异。

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

先生成三联设计预览给我检查，不要把预览图裁切为正式资源，也不要接入前端。
```

设计预览确认后继续发：

```text
这组成就徽章设计满意。按照流程分别生成 tier-1/2/3 的纯 #ff00ff 色键图，确保徽章为开放式结构；转换成 512×512 透明 PNG 后，提供深色和浅色背景预览，不要把背景光效烘焙进徽章。
```
