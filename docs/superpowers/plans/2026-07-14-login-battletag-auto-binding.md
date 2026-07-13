# Login BattleTag Auto-Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 登录网易大神时从当前页面自动识别本人 BattleTag，安全完成当前目标绑定，并彻底移除任意 BattleTag 手工输入入口。

**Architecture:** 复用现有 CDP 页面文字采集，在同一轮候选中提取 `role_id` 与带“战网昵称”标签的 BattleTag；凭据换取成功后由新的 `login_target` 模块协调自动绑定、相同目标复用、不同目标确认和失败重试。唯一当前目标记录绑定来源 `role_id`，桌面同步入口在发请求前校验登录账号与目标关联，避免多账号切换时把新 token 与旧目标组合。

**Tech Stack:** Rust、Tauri 2、Serde、Reqwest、React、TypeScript、SQLite、Node/tsx 源码契约测试

---

## 执行前提

- 开始 Task 1 前先运行 `git status --short`，保留所有与本功能无关的用户改动。
- 当前工作树经常非干净；执行时先使用 `superpowers:using-git-worktrees` 建立隔离工作区，或明确证明当前工作区没有会与本计划重叠的未提交修改。
- 自动化测试只使用合成账号、BattleTag 和 token；不得人工读取 app data 凭据文件或浏览器 profile。

## 文件职责

- `apps/owstats-desktop/src-tauri/src/dashen_auth.rs`：从页面候选提取 `role_id` 和 BattleTag。
- `apps/owstats-desktop/src-tauri/src/credentials.rs`：维护单次登录轮次，保证凭据与 BattleTag 成对返回。
- `apps/owstats-desktop/src-tauri/src/player_target.rs`：读写唯一当前目标及其来源登录 `role_id`。
- `apps/owstats-desktop/src-tauri/src/login_target.rs`：新增；协调自动绑定、切换确认、失败重试和同步资格。
- `apps/owstats-desktop/src-tauri/src/models.rs`：定义无 secret 的前端 DTO。
- `apps/owstats-desktop/src-tauri/src/commands.rs`、`src/lib.rs`：暴露无任意 BattleTag 参数的安全命令。
- `apps/owstats-desktop/src/shared/types.ts`、`src/shared/api.ts`、`src/pages/OnboardingPage.tsx`：自动识别状态 UI。
- `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`：页面解析与登录轮次测试。
- `apps/owstats-desktop/src-tauri/tests/login_target.rs`：新增；目标协调、账号切换、同步资格和隐私测试。

### Task 1: 提取带标签的 BattleTag

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_auth.rs:7-72`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`

- [ ] **Step 1: 写 BattleTag 提取失败测试**

```rust
#[test]
fn extracts_only_labeled_battletag_from_page_candidates() {
    let candidates = vec![
        "推荐玩家 Other#9999".to_string(),
        "战网昵称： 风偏偏呢#5479\n切换".to_string(),
    ];
    assert_eq!(
        extract_battle_tag(&candidates),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
}

#[test]
fn battletag_extraction_rejects_conflicts_and_unlabeled_names() {
    assert_eq!(
        extract_battle_tag(&[
            "战网昵称：First#1111".to_string(),
            "战网昵称: Second#2222".to_string(),
        ]),
        BattleTagExtraction::Conflict
    );
    assert_eq!(
        extract_battle_tag(&["好友 Other#9999".to_string()]),
        BattleTagExtraction::Missing
    );
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase4_login_redesign battletag -- --nocapture`

Expected: FAIL，提示 `extract_battle_tag` 或 `BattleTagExtraction` 尚未定义。

- [ ] **Step 3: 实现锚点解析器**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleTagExtraction {
    Missing,
    Found(String),
    Conflict,
}

pub fn extract_battle_tag(candidates: &[String]) -> BattleTagExtraction {
    let mut found: Option<String> = None;
    for candidate in candidates {
        for line in candidate.lines() {
            let line = line.trim();
            let Some(value) = line
                .strip_prefix("战网昵称：")
                .or_else(|| line.strip_prefix("战网昵称:"))
            else { continue };
            let Some(tag) = labeled_battletag_value(value) else { continue };
            if found.as_deref().is_some_and(|current| current != tag) {
                return BattleTagExtraction::Conflict;
            }
            found = Some(tag.to_string());
        }
    }
    found.map_or(BattleTagExtraction::Missing, BattleTagExtraction::Found)
}

fn labeled_battletag_value(value: &str) -> Option<&str> {
    let token = value.split_whitespace().next()?.trim();
    let (name, suffix) = token.rsplit_once('#')?;
    if name.trim().is_empty() || suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(token)
}
```

- [ ] **Step 4: 运行全部登录解析测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase4_login_redesign -- --nocapture`

Expected: PASS；现有 `role_id/getReportToken` 测试不回归。

- [ ] **Step 5: 提交解析器**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/dashen_auth.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs
git commit -m "feat: extract current BattleTag from login page"
```

### Task 2: 保证 BattleTag 与凭据来自同一登录轮次

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs:44-195`
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_auth.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`

- [ ] **Step 1: 写登录身份跟踪失败测试**

```rust
#[test]
fn login_identity_tracker_never_pairs_new_role_with_old_battletag() {
    let mut tracker = LoginIdentityTracker::default();
    tracker.observe(Some("101".into()), BattleTagExtraction::Found("First#1111".into()));
    assert_eq!(tracker.complete(), Some(("101", "First#1111")));

    tracker.observe(None, BattleTagExtraction::Missing);
    assert_eq!(tracker.complete(), Some(("101", "First#1111")));

    tracker.observe(Some("202".into()), BattleTagExtraction::Missing);
    assert_eq!(tracker.role_id(), Some("202"));
    assert_eq!(tracker.battle_tag(), None);
    assert_eq!(tracker.complete(), None);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase4_login_redesign login_identity_tracker -- --nocapture`

Expected: FAIL，提示 `LoginIdentityTracker` 尚未定义。

- [ ] **Step 3: 实现纯状态跟踪器和浏览器结果类型**

```rust
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoginIdentityTracker {
    role_id: Option<String>,
    battle_tag: Option<String>,
}

impl LoginIdentityTracker {
    pub fn observe(&mut self, role_id: Option<String>, battle_tag: BattleTagExtraction) {
        if let Some(role_id) = role_id {
            if self.role_id.as_deref() != Some(role_id.as_str()) {
                self.role_id = Some(role_id);
                self.battle_tag = None;
            }
        }
        self.battle_tag = match battle_tag {
            BattleTagExtraction::Found(value) if self.role_id.is_some() => Some(value),
            BattleTagExtraction::Conflict => None,
            BattleTagExtraction::Missing => self.battle_tag.clone(),
            BattleTagExtraction::Found(_) => None,
        };
    }
    pub fn role_id(&self) -> Option<&str> { self.role_id.as_deref() }
    pub fn battle_tag(&self) -> Option<&str> { self.battle_tag.as_deref() }
    pub fn complete(&self) -> Option<(&str, &str)> { Some((self.role_id()?, self.battle_tag()?)) }
}

pub struct BrowserLoginOutcome {
    pub credentials: DashenCredentials,
    pub detected_bnet: Option<String>,
}

struct PageIdentity {
    role_id: Option<String>,
    battle_tag: BattleTagExtraction,
}

async fn collect_page_identity(page: &mut CdpPage) -> Option<PageIdentity> {
    let candidates = page.evaluate(&build_collect_candidates_script(), false).await.ok()?;
    let texts = candidates.as_array()?.iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    Some(PageIdentity {
        role_id: extract_role_id(&texts),
        battle_tag: extract_battle_tag(&texts),
    })
}
```

新增 `start_dashen_login_capture`，完整复用现有浏览器启动、凭据原子保存和进度事件，只把内部返回值改为 `BrowserLoginOutcome`。暂时保留现有 `start_dashen_login` 包装器并让它调用 capture 后返回旧 DTO，保证本任务提交仍可编译。导航到签名页可以在暂时读不到 `role_id` 时保留上一页已经配对的身份，但观察到不同 `role_id` 时必须先清空旧 BattleTag。候选数组和页面全文不得进入结果、日志或错误。

- [ ] **Step 4: 运行测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase4_login_redesign -- --nocapture`

Expected: PASS，测试输出不含页面全文或 secret。

- [ ] **Step 5: 提交登录轮次改造**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/src/dashen_auth.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs
git commit -m "feat: pair login credentials with detected BattleTag"
```

### Task 3: 给唯一当前目标增加登录账号关联

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/player_target.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/bin/fetch_bnet_season_json.rs:203-218`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs:167-203`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/login_target.rs`

- [ ] **Step 1: 写目标关联失败测试**

```rust
use owstats_desktop_lib::player_target::{target_matches_login_role, PlayerTarget};

#[test]
fn target_is_syncable_only_for_the_role_that_bound_it() {
    let target = PlayerTarget {
        input_bnet: "Tester#12345".into(),
        resolved_bnet: Some("Tester#12345".into()),
        customer_token: "synthetic-customer-token".into(),
        bound_at: "2026-07-14T00:00:00Z".into(),
        source_role_id: Some(101),
    };
    assert!(target_matches_login_role(&target, 101));
    assert!(!target_matches_login_role(&target, 202));
}

#[test]
fn legacy_target_without_source_role_requires_relogin() {
    let target: PlayerTarget = serde_json::from_str(
        r#"{"inputBnet":"Tester#12345","resolvedBnet":"Tester#12345","customerToken":"synthetic","boundAt":"2026-07-14T00:00:00Z"}"#,
    ).unwrap();
    assert_eq!(target.source_role_id, None);
    assert!(!target_matches_login_role(&target, 101));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test login_target -- --nocapture`

Expected: FAIL，提示 `source_role_id` 或匹配函数不存在。

- [ ] **Step 3: 扩展目标格式并保持旧 JSON 可解析**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTarget {
    pub input_bnet: String,
    pub resolved_bnet: Option<String>,
    pub customer_token: String,
    pub bound_at: String,
    #[serde(default)]
    pub source_role_id: Option<i64>,
}

pub fn target_matches_login_role(target: &PlayerTarget, role_id: i64) -> bool {
    target.source_role_id == Some(role_id)
}
```

把 `target_from_search_payload` 改为接收 `source_role_id: i64` 并保存 `Some(source_role_id)`。更新六处 `PlayerTarget { ... }` 构造；诊断 CLI 使用 `source_role_id: None`。Task 5 删除旧命令前，先让现有 `bind_player_target` 传入 `saved.role_id`，保证本任务独立编译。

- [ ] **Step 4: 运行目标与合成抓取测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test login_target --test phase8_real_fetch -- --nocapture`

Expected: PASS；旧目标 JSON 可读但匹配结果为 false。

- [ ] **Step 5: 提交目标关联字段**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/player_target.rs apps/owstats-desktop/src-tauri/src/bin/fetch_bnet_season_json.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src-tauri/tests/login_target.rs
git commit -m "feat: associate current target with login role"
```

### Task 4: 实现自动绑定与账号切换状态机

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/login_target.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs:519-545,745-750`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/login_target.rs`

- [ ] **Step 1: 写纯决策和运行期状态失败测试**

```rust
#[test]
fn target_action_depends_on_the_single_current_target() {
    assert_eq!(decide_target_action("Current#1000", None), TargetAction::AutoBind);
    assert_eq!(
        decide_target_action("Current#1000", Some("current#1000")),
        TargetAction::KeepCurrent
    );
    assert_eq!(
        decide_target_action("Second#2000", Some("Current#1000")),
        TargetAction::ConfirmSwitch
    );
}

#[test]
fn keeping_a_different_target_disables_sync() {
    let coordinator = LoginTargetCoordinator::default();
    coordinator.set_switch_required(202, "Second#2000", "Current#1000");
    let status = coordinator.keep_current().unwrap();
    assert_eq!(status.state, "mismatchKept");
    assert!(!status.sync_allowed);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test login_target -- --nocapture`

Expected: FAIL，提示 `login_target` 模块与状态类型不存在。

- [ ] **Step 3: 定义无 secret DTO 和协调器**

在 `models.rs` 增加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginTargetStatus {
    pub state: String,
    pub detected_bnet: Option<String>,
    pub current_bnet: Option<String>,
    pub message: String,
    pub sync_allowed: bool,
    pub can_confirm_switch: bool,
    pub can_retry: bool,
}

pub struct CredentialLoginResult {
    pub status: CredentialStatus,
    pub target: LoginTargetStatus,
}
```

在新模块中增加：

```rust
#[derive(Clone, Default)]
pub struct LoginTargetCoordinator {
    session: Arc<Mutex<Option<LoginTargetSession>>>,
}

#[derive(Debug, Clone)]
struct LoginTargetSession {
    source_role_id: i64,
    detected_bnet: Option<String>,
    status: LoginTargetStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetAction { AutoBind, KeepCurrent, ConfirmSwitch }

pub fn decide_target_action(detected: &str, current: Option<&str>) -> TargetAction {
    match current {
        None => TargetAction::AutoBind,
        Some(value) if normalize_bnet_for_compare(value) == normalize_bnet_for_compare(detected) => TargetAction::KeepCurrent,
        Some(_) => TargetAction::ConfirmSwitch,
    }
}

fn normalize_bnet_for_compare(value: &str) -> String {
    value.trim().replace('＃', "#").to_lowercase()
}
```

协调器只保存 `role_id`、BattleTag 和无 secret 状态；不得保存账号 token 或 `customerToken`。实现 `clear`、`keep_current`、`status` 和用新登录轮次整体替换旧 session。

- [ ] **Step 4: 实现可录制测试的绑定函数**

```rust
pub async fn bind_detected_target_with_base_url(
    credentials: &DashenCredentials,
    detected_bnet: &str,
    target_path: &Path,
    root_base_url: &str,
) -> Result<PlayerTargetStatus> {
    let client = DashenClient::new(
        crate::dashen_client::DashenCredentials {
            role_id: credentials.role_id,
            token: credentials.token.clone(),
            dts: credentials.dts.as_deref().and_then(|value| value.parse().ok()),
            server: credentials.server,
        },
        root_base_url.to_string(),
    )?;
    let payload = client.search_bnet_account(detected_bnet).await?;
    let target = target_from_search_payload(
        detected_bnet,
        &payload,
        credentials.role_id,
        &Utc::now().to_rfc3339(),
    )?;
    client.query_card(&target.customer_token).await?;
    save_player_target_for_path(target_path, &target)?;
    player_target_status_for_path(target_path)
}
```

实现 `reconcile_detected_target`、`confirm_switch`、`retry_binding` 和 `status_or_persisted`。绑定失败转换为 `bindFailed` 状态，保留可重试 BattleTag，但登录仍成功。错误摘要必须经过现有递归/文本脱敏边界。

核心状态转移必须按下列分支落地，不允许前端传入 BattleTag：

```rust
pub async fn reconcile_detected_target(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    detected_bnet: Option<&str>,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    let Some(detected_bnet) = detected_bnet else {
        return coordinator.replace_unidentified(credentials.role_id);
    };
    let current = read_player_target_for_path(target_path).ok();
    match current.as_ref() {
        None => bind_or_record_failure(coordinator, credentials, detected_bnet, target_path, root_base_url).await,
        Some(target)
            if target_display_bnet(target).eq_ignore_ascii_case(detected_bnet)
                && target_matches_login_role(target, credentials.role_id) =>
        {
            coordinator.replace_ready(credentials.role_id, detected_bnet, target_display_bnet(target))
        }
        Some(target) if target_display_bnet(target).eq_ignore_ascii_case(detected_bnet) => {
            // 旧目标或来源不匹配：重新解析并保存，写入本轮 role_id。
            bind_or_record_failure(coordinator, credentials, detected_bnet, target_path, root_base_url).await
        }
        Some(target) => coordinator.replace_switch_required(
            credentials.role_id,
            detected_bnet,
            target_display_bnet(target),
        ),
    }
}

async fn bind_or_record_failure(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    detected_bnet: &str,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    match bind_detected_target_with_base_url(credentials, detected_bnet, target_path, root_base_url).await {
        Ok(status) => coordinator.replace_ready(
            credentials.role_id,
            detected_bnet,
            status.resolved_bnet.as_deref().unwrap_or(detected_bnet),
        ),
        Err(error) => coordinator.replace_bind_failed(
            credentials.role_id,
            detected_bnet,
            &redacted_login_error(&error.to_string()),
        ),
    }
}
```

`status_or_persisted` 在应用重启后读取凭据和目标：二者 role 匹配返回 `ready`；旧目标缺少 `source_role_id` 或 role 不同返回 `verificationRequired` 且 `sync_allowed=false`。

- [ ] **Step 5: 运行状态与录制 HTTP 测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test login_target -- --nocapture`

Expected: PASS；录制请求先访问 `/searchBnetAccount` 再访问 `/customer/queryCard`，目标文件不含账号 token。

- [ ] **Step 6: 提交自动绑定模块**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/login_target.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/login_target.rs
git commit -m "feat: coordinate automatic current-account binding"
```

### Task 5: 收紧 Tauri 命令面并阻止账号错配同步

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs:148-214,313-339`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs:1-80`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs:519-545`
- Test: `apps/owstats-desktop/src-tauri/tests/login_target.rs`
- Test: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`

- [ ] **Step 1: 写命令面和同步门禁失败测试**

```ts
assert.doesNotMatch(api, /bind_player_target/, "product API must not accept arbitrary BattleTag binding");
assert.match(api, /get_login_target_status/, "product API exposes safe target status");
assert.match(api, /confirm_login_target_switch/, "product API confirms detected account switch");
assert.match(api, /keep_current_player_target/, "product API can preserve the old target");
assert.match(api, /retry_login_target_binding/, "product API retries without a BattleTag argument");
```

Rust 测试覆盖：来源 role 相同允许同步、不同拒绝、旧目标缺少来源拒绝；错误不得包含 role ID、BattleTag 或 token。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd apps/owstats-desktop; npx tsx scripts/phase8RealFetch.test.ts`

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test login_target sync -- --nocapture`

Expected: 两项均 FAIL，旧任意绑定命令仍暴露且同步门禁不存在。

- [ ] **Step 3: 让登录命令完成自动协调**

```rust
#[tauri::command]
pub async fn start_dashen_login(
    app: AppHandle,
    coordinator: State<'_, LoginTargetCoordinator>,
) -> Result<CredentialLoginResult, String> {
    let outcome = crate::credentials::start_dashen_login_capture(app.clone()).await.map_err(to_message)?;
    let target = crate::login_target::reconcile_detected_target(
        &app,
        &coordinator,
        &outcome.credentials,
        outcome.detected_bnet.as_deref(),
    ).await.map_err(to_message)?;
    Ok(CredentialLoginResult {
        status: crate::credentials::get_credential_status(&app).map_err(to_message)?,
        target,
    })
}
```

自动绑定失败必须返回 `bindFailed` target 状态，不能让整个登录命令失败。

- [ ] **Step 4: 删除任意绑定命令并注册安全命令**

删除 `BindPlayerTargetRequest`、`BindPlayerTargetResult`、`bind_player_target` command 和 handler。新增不接受 BattleTag 参数的：

```rust
pub fn get_login_target_status(
    app: AppHandle,
    coordinator: State<'_, LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String>;

pub async fn confirm_login_target_switch(
    app: AppHandle,
    coordinator: State<'_, LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String>;

pub fn keep_current_player_target(
    coordinator: State<'_, LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String>;

pub async fn retry_login_target_binding(
    app: AppHandle,
    coordinator: State<'_, LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String>;
```

`logout` 和每个新登录轮次调用 `coordinator.clear()`。清除目标后只允许使用协调器中同轮识别值重试。

把 `ClearPlayerTargetResult` 调整为携带 `target: LoginTargetStatus`。清除成功后，如果协调器仍保存本轮可靠 BattleTag，返回 `bindFailed/canRetry=true`；否则返回 `unidentified/canRetry=false`。前端永远不能通过清除操作获得手工输入入口。

- [ ] **Step 5: 在桌面同步入口校验来源账号**

```rust
crate::login_target::ensure_sync_allowed(
    &credential_path,
    &target_path,
    coordinator.target_session(),
).map_err(to_message)?;
```

持久化目标的 `source_role_id` 必须与当前凭据一致；运行期 `switchRequired`、`mismatchKept`、`unidentified` 或 `bindFailed` 均拒绝同步。错误固定为“当前登录账号与同步目标尚未确认，请重新登录或确认切换”，不带任何标识值。

- [ ] **Step 6: 运行后端与命令面测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test login_target --test phase8_real_fetch -- --nocapture`

Run: `cd apps/owstats-desktop; npx tsx scripts/phase8RealFetch.test.ts`

Expected: PASS；源码中不再注册或调用 `bind_player_target`。

- [ ] **Step 7: 提交命令收紧与同步门禁**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/tests/login_target.rs apps/owstats-desktop/scripts/phase8RealFetch.test.ts
git commit -m "feat: gate sync on detected login target"
```

### Task 6: 将指挥台改为自动识别状态 UI

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts:59-85`
- Modify: `apps/owstats-desktop/src/shared/api.ts:104-122`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx:18-220,329-363`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Test: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`

- [ ] **Step 1: 写 UI 源码契约失败测试**

```ts
assertEqual(onboardingSource.includes('aria-label="BattleTag"'), false, "command center removes BattleTag input");
assertEqual(onboardingSource.includes("bindPlayerTarget"), false, "command center cannot bind arbitrary targets");
assertEqual(onboardingSource.includes("切换到此账号"), true, "command center confirms detected switches");
assertEqual(onboardingSource.includes("保留原目标"), true, "command center can preserve the old target");
assertEqual(onboardingSource.includes("重新绑定"), true, "command center retries the detected target");
assertEqual(onboardingSource.includes("重新识别"), true, "command center guides missing BattleTag recovery");
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd apps/owstats-desktop; npx tsx scripts/phase6Shell.test.ts; npx tsx scripts/phase8RealFetch.test.ts`

Expected: FAIL，现有输入框、`bindPlayerTarget` 和任意目标文案仍存在。

- [ ] **Step 3: 更新前端类型与 API**

```ts
export type LoginTargetStatus = {
  state: "ready" | "switchRequired" | "mismatchKept" | "bindFailed" | "unidentified" | "verificationRequired" | string;
  detectedBnet: string | null;
  currentBnet: string | null;
  message: string;
  syncAllowed: boolean;
  canConfirmSwitch: boolean;
  canRetry: boolean;
};

export type CredentialLoginResult = {
  status: CredentialStatus;
  target: LoginTargetStatus;
};
```

删除 `bindPlayerTarget(bnet)`，新增四个无 BattleTag 参数 API，与 Task 5 命令一一对应。

- [ ] **Step 4: 重写目标状态区域**

删除 `battleTag` state、`runBindTarget`、输入框和“更换目标”。`runLogin` 直接保存 `result.target`。按状态渲染：

```tsx
{loginTargetStatus?.state === "switchRequired" && (
  <div className="syncTargetDecision">
    <button className="primaryButton" type="button" onClick={runConfirmTargetSwitch}>切换到此账号</button>
    <button className="ghostButton" type="button" onClick={runKeepCurrentTarget}>保留原目标</button>
  </div>
)}
{loginTargetStatus?.state === "bindFailed" && (
  <button className="primaryButton" type="button" onClick={runRetryTargetBinding}>重新绑定</button>
)}
{loginTargetStatus?.state === "unidentified" && (
  <button className="primaryButton" type="button" onClick={runLogin}>重新识别</button>
)}
```

目标摘要只显示 `currentBnet`、`detectedBnet` 和中文 `message`。同步资格改为：

```tsx
credentialReady={credentialReady && Boolean(loginTargetStatus?.syncAllowed)}
```

保留“清除”按钮；`runClearTarget` 使用新的 `result.target` 更新状态。清除后只有 `canRetry=true` 才显示“重新绑定”，否则显示“重新识别”。

- [ ] **Step 5: 增加最小响应式样式**

```css
.syncTargetDecision {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}
```

只新增确认按钮容器，不重排指挥台其他模块。

- [ ] **Step 6: 运行前端测试和构建**

Run: `cd apps/owstats-desktop; npx tsx scripts/phase6Shell.test.ts; npx tsx scripts/phase8RealFetch.test.ts; npm test; npm run build`

Expected: 全部 PASS；构建产物不包含 BattleTag 输入框或 `bind_player_target` 调用。

- [ ] **Step 7: 提交指挥台改造**

```powershell
git add -- apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/phase8RealFetch.test.ts
git commit -m "feat: show automatic login target states"
```

### Task 7: 完整隐私验证与真实验收交接

**Files:**
- Modify: `docs/current-development-status.md`
- Modify: `docs/superpowers/specs/main-thread-log.md`
- Test: all files changed above

- [ ] **Step 1: 运行产品入口与隐私扫描**

Run:

```powershell
rg -n 'bind_player_target|aria-label="BattleTag"|placeholder="BattleTag|请输入 BattleTag|同步任意' apps/owstats-desktop/src apps/owstats-desktop/src-tauri/src apps/owstats-desktop/scripts
```

Expected: 无产品入口命中；历史 fixture 如有命中，逐条确认不被 Tauri handler 或前端 API 引用。

Run:

```powershell
rg -n 'customerToken|customer_token|authorization|credential|cookie|token' apps/owstats-desktop/src-tauri/tests/login_target.rs
```

Expected: 只命中合成值和“不泄露”断言，不包含真实值。

- [ ] **Step 2: 运行后端定向与全量验证**

Run:

```powershell
cd apps/owstats-desktop/src-tauri
cargo fmt --check
cargo test --test phase4_login_redesign --test login_target --test phase8_real_fetch
cargo test --tests
```

Expected: 全部退出码 0。

- [ ] **Step 3: 运行前端全量验证**

Run:

```powershell
cd apps/owstats-desktop
npm test
npm run build
```

Expected: 全部退出码 0，Vite 构建成功。

- [ ] **Step 4: 运行仓库验证和差异检查**

Run:

```powershell
cd D:\Codex\OW_data\owstats-local
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -ChangedOnly
git diff --check
git status --short
```

Expected: 验证退出码 0；`git diff --check` 无错误；状态只包含本功能预期文件以及执行前已存在、未触碰的用户改动。

- [ ] **Step 5: 更新状态文档**

写入以下事实，不记录真实账号名、role ID 或 secret：

```markdown
- 网易大神登录现在从带“战网昵称”标签的页面文字自动识别当前 BattleTag。
- 产品不再暴露任意 BattleTag 输入或绑定命令。
- 当前目标记录单一来源账号关联；账号与目标未确认时禁用同步。
- 自动化验证使用合成凭据，未读取真实浏览器 profile 或真实凭据。
- 真实多账号切换仍需用户在桌面端完成手工验收。
```

- [ ] **Step 6: 提交验证文档**

```powershell
git add -- docs/current-development-status.md docs/superpowers/specs/main-thread-log.md
git commit -m "docs: record automatic BattleTag binding verification"
```

- [ ] **Step 7: 用户执行真实桌面验收**

执行线程只记录通过/失败，不记录账号值：

1. 无目标时登录，自动绑定并允许同步。
2. 同账号重新登录，不重复绑定。
3. 浏览器点击“切换”并登录第二个账号，应用要求确认。
4. 选择“保留原目标”，同步保持禁用。
5. 重新登录第二个账号并确认切换，目标更新且同步恢复。
6. 进入不显示“战网昵称”的页面，登录凭据成功但目标识别失败，只显示“重新识别”。
7. 同步一个赛季，确认正常落盘和导入。

- [ ] **Step 8: 说明发布验证状态**

如果未执行 `npm run tauri build` 和安装后烟测，最终报告必须明确写“未验证安装版”；不得把前端构建或开发版登录验收表述为发布验证完成。
