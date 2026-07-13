# 角斗未映射素材 GUID 兜底设计

## 目标

当角斗详情中的 `traitGuids` 或 `modGuids` 无法在 `assets/stadium/manifest.json` 中解析时，桌面应用显示该原始 GUID，而不是“待补充特质”或“待补充异能”。

## 范围

- 仅改动紧凑角斗详情的异能和装备槽位。
- 已解析项目继续显示既有中文名称、图标和描述。
- 未解析项目不显示图标；异能和装备槽位名称、tooltip 标题和无障碍标签统一使用原始 GUID。
- 不新增持久化字段、不改变导入数据、不影响普通对局、英雄、地图或玩家信息。

## 实现

1. 在 `CompactArenaSlotGroup` 中，以当前槽位的 `guid` 作为资源解析失败时的回退文本。
2. 保持空槽位没有文本；只有存在 GUID 但无映射时才显示该 GUID。
3. 移除该组件调用点中的“待补充”fallback 文本参数。
4. 为未映射 GUID 添加前端资源测试，并保留现有安全路径与已映射资源测试。
5. 在 `AGENTS.md` 中记录仅限 `traitGuids` 与 `modGuids` 的发布 UI 例外，防止后续回退为“待补充”文案。

## 验证

- 新测试先断言未映射异能/装备返回并显示原始 GUID。
- `npm run test:resources`、`npm run build` 和 `git diff --check` 通过。
