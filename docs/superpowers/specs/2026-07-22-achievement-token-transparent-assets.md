# 成就徽章透明底资源流程

## 目标

将每组成就的 I、II、III 级徽章替换为真正带透明通道的 512×512 PNG。保留徽章身份、主体符号、等级配色和金属材质；移除外部矩形背景、投影、外发光及徽章背后的整块暗色圆盘。

## 生成规则

每枚正式资源单独使用原图作为编辑目标，不从组合图裁切。固定采用 `background-extraction` 流程：

- 背景为完全均匀的 `#ff00ff`，不得包含渐变、纹理、阴影、反射或暗角。
- 徽章外圈、主体符号、桂冠、宝石和附着在实体零件上的珐琅色必须保留。
- 实体零件之间的开放区域必须直接露出 `#ff00ff`，不得重新填充黑色或深色底盘。
- 禁止添加文字、水印、新符号、粒子、光晕或额外装饰。
- 每张图只包含一枚正面居中的徽章，四周留出均匀空间。

如果初稿仍保留暗色底盘，使用定向提示再次生成，明确指出哪些圆环、叶片或主体符号之间必须露出色键背景。若色键转透明后金色偏绿或出现洋红残边，重新生成无外发光、硬边缘版本，不接受通过调色掩盖污染。

## 后处理

将通过视觉检查的色键图按如下结构保存：

```text
tmp/achievement-transparent/<group>/tier-1-key.png
tmp/achievement-transparent/<group>/tier-2-key.png
tmp/achievement-transparent/<group>/tier-3-key.png
```

随后在仓库根目录运行：

```powershell
python scripts/process_achievement_token_transparency.py `
  --group <group> `
  --input-dir tmp/achievement-transparent/<group>
```

脚本固定使用边界自动取色、软遮罩、透明阈值 12、不透明阈值 220 和去色键污染，然后把主体等比缩放至不超过 480×480，居中写入透明的 512×512 画布，并覆盖 `assets/achievements/token-style/<group>-token-tier-N.png`。

## 验收标准

- 三枚正式资源均为 512×512 RGBA PNG。
- Alpha 通道同时包含 0 和 255，四角 Alpha 必须为 0。
- 深色与浅色背景上均无矩形底、暗色圆盘、洋红描边或异常偏色。
- I、II、III 仍能一眼识别为同一成就的三个等级，并保持铜色、银蓝色、金色层级。
- 运行 `npm run prepare:assets`、资源测试、前端构建和 `git diff --check`。
