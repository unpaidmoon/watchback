# AI Roast Output Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让真实 DeepSeek JSON 输出在安全校验不放宽的前提下可被稳定解析，并让一次自动重试能纠正具体格式问题。

**Architecture:** 校验器负责安全地剥离完整 JSON 代码围栏并返回现有结构化错误；客户端把错误映射为不含原始内容的固定修正提示，重新构造第二次请求。所有对外错误、数据持久化和隐私边界保持不变。

**Tech Stack:** Rust, serde_json, reqwest, Tokio

---

### Task 1: 兼容完整 JSON 代码围栏

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`

- [ ] 写一个合法 JSON 被 `json` 代码围栏完整包裹的失败测试。
- [ ] 运行精确测试，确认因 `InvalidJson` 失败。
- [ ] 实现只接受完整围栏的输入规范化；混合解释文字仍拒绝。
- [ ] 运行校验器相关测试并确认通过。

### Task 2: 让第二次请求针对失败类别纠正

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/client.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`

- [ ] 写合成 HTTP 测试：第一次返回句数错误，第二次请求必须包含固定句数修正提示且不包含第一次正文。
- [ ] 运行精确测试，确认第二次请求仍与第一次相同而失败。
- [ ] 将 `RoastValidationError` 映射为固定修正提示，并只在校验失败时追加到第二次请求。
- [ ] 保持响应 envelope 解析失败的第二次请求使用通用 JSON 修正提示。
- [ ] 运行 `cargo test --test ai_roast_client`。

### Task 3: 验证真实链路与回归

**Files:**
- No production file changes expected.

- [ ] 在不输出凭据、响应、ID、路径或真实数据的诊断入口中调用安全存储 Key 和当前账号的一场可生成对局。
- [ ] 仅报告成功/匿名失败类别与耗时；失败时依据类别补充最小合成测试并回到 Task 2。
- [ ] 运行 `cargo fmt --check`、`cargo test --tests`、`git diff --check` 和 `git status --short`。
