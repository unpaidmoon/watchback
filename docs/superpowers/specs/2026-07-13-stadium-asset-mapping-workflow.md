# 角斗装备与异能人工映射协作流程

## 编辑主表

`2026-07-13-stadium-asset-mapping.xlsx` 是人工编辑主表。它按“中立”和英雄分 sheet，且只使用以下五列：

1. GUID：十进制文本，必填，不得改为科学计数法。
2. 装备/异能：用于校对类型。
3. 名称：可编辑；未确认时留空。
4. 描述：可编辑；未确认时留空。
5. 资产路径：已打包资产使用 `assets/stadium/...`；待导入的新图标可先填写本地源文件路径。

## 同步方式

编辑完成后，用户发送“同步角斗映射表”。同步任务按十进制 GUID 识别条目并转换为十六进制，更新 `assets/stadium/manifest.json`：

- 名称和描述直接写入对应 manifest 条目；空单元格不覆盖现有已知值。
- 已存在的正式资产路径必须存在于 `assets/` 并写入 manifest。
- 新图标源文件先转换或复制至 `assets/stadium/`；manifest 和主表最终都写入正式相对路径，而非开发机绝对路径。
- 主表中不存在于 manifest 的 GUID 作为新增候选，必须先验证英雄 sheet、类型与图标文件，再加入运行时映射。

## 验证与回写

每次同步后执行：

```powershell
cd apps\owstats-desktop
npm run prepare:assets
npm run test:resources
npm run build
```

同步成功后，将 manifest 的标准化结果回写到主表，GUID 保持十进制文本。这样桌面应用中的未映射 GUID、主表中的 GUID 与用户截图可直接对照。
