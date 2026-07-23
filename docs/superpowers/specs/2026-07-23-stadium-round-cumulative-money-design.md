# 角斗单回合累积资金显示设计

## 目标

在对局详情的角斗对局中，每个“回合-N”页签只显示该回合结束后的累积资金，不再显示“起始资金 -> 结束资金”的单回合变化。角斗汇总页签维持现有显示规则。

## 实现范围

- 保留资金图标、累积资金数值和“本场资金”标签。
- 回合模式优先显示 `endWorth`；当 `endWorth` 缺失时，回退显示 `worth`；两者都缺失时继续显示现有中文兜底。
- 汇总模式继续优先显示 `endWorth`，并在缺失时回退到 `worth`。
- 不修改后端、SQLite、同步数据、上游原始字段或其他对局详情。

## 实现位置

修改 `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx` 中的资金格式化逻辑，不在 JSX 或数据转换层增加重复分支。

## 验证

- 先更新 `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`，断言回合模式不再包含资金变化箭头且优先使用累积资金，并确认测试因旧逻辑失败。
- 再进行最小实现，使针对性测试通过。
- 运行相关前端测试、`git diff --check`，并检查工作树只新增本任务的预期修改。
