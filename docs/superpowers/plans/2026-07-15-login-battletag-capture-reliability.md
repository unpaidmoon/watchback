# Login BattleTag Capture Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reliably extract the visible current-account BattleTag from split DOM layouts and pair it with the only credential role successfully confirmed during the same browser-login round.

**Architecture:** Extend the existing CDP collection script with a bounded `battleTagCandidates` group gathered from labeled DOM text nodes and accessible same-origin frames. Keep final candidate validation and conflict handling in Rust, then relax only the same-round pairing boundary so a stable BattleTag can use the credential role confirmed by the existing token exchange when current-page role evidence is absent but not conflicting.

**Tech Stack:** Rust, Tauri 2, Chrome DevTools Protocol `Runtime.evaluate`, existing Rust integration tests, Playwright CLI for synthetic DOM acceptance.

---

## File map

- Modify `apps/owstats-desktop/src-tauri/src/dashen_auth.rs`: build the bounded DOM candidate collector and validate candidate values in Rust.
- Modify `apps/owstats-desktop/src-tauri/src/credentials.rs`: decode `battleTagCandidates` and pair a stable candidate with the same-round credential role.
- Modify `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`: add extraction, script-contract, pairing, reset, and privacy regression coverage.
- Modify `apps/owstats-desktop/scripts/phase8RealFetch.test.ts` only if a product-surface assertion is needed to lock the absence of manual BattleTag input; no UI behavior change is expected.

Do not modify `apps/owstats-desktop/src-tauri/Cargo.toml`; it contains an unrelated parallel workspace change.

### Task 1: Validate bounded BattleTag candidate groups in Rust

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_auth.rs:1-55`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs:1-145`

- [ ] **Step 1: Write failing candidate-validation tests**

Import `extract_battle_tag_candidates` and add tests that exercise values rather than page-wide lines:

```rust
#[test]
fn battletag_candidates_accept_split_dom_values_and_deduplicate() {
    assert_eq!(
        extract_battle_tag_candidates(
            &["风偏偏呢#5479".to_string(), " 风偏偏呢#5479 ".to_string()],
            false,
        ),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
}

#[test]
fn battletag_candidates_reject_conflicts_truncation_and_invalid_values() {
    assert_eq!(
        extract_battle_tag_candidates(
            &["First#1111".to_string(), "Second#2222".to_string()],
            false,
        ),
        BattleTagExtraction::Conflict
    );
    assert_eq!(
        extract_battle_tag_candidates(&["First#1111".to_string()], true),
        BattleTagExtraction::Conflict
    );
    assert_eq!(
        extract_battle_tag_candidates(
            &["MissingHash".to_string(), "Invalid#12A".to_string()],
            false,
        ),
        BattleTagExtraction::Missing
    );
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run from `apps/owstats-desktop/src-tauri`:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo test --test phase4_login_redesign battletag_candidates -- --nocapture
```

Expected: compilation fails because `extract_battle_tag_candidates` does not exist.

- [ ] **Step 3: Implement minimal candidate validation**

Add a value validator and candidate-group function while keeping the old page-text wrapper until the browser collector is wired:

```rust
pub fn extract_battle_tag_candidates(
    candidates: &[String],
    truncated: bool,
) -> BattleTagExtraction {
    if truncated {
        return BattleTagExtraction::Conflict;
    }
    let mut found = BTreeSet::new();
    for candidate in candidates {
        let value = candidate.trim();
        let Some((name, suffix)) = value.split_once('#') else {
            continue;
        };
        if name.is_empty()
            || suffix.is_empty()
            || suffix.contains('#')
            || !suffix.chars().all(|ch| ch.is_ascii_digit())
        {
            continue;
        }
        found.insert(value.to_string());
    }
    match found.len() {
        0 => BattleTagExtraction::Missing,
        1 => BattleTagExtraction::Found(found.into_iter().next().unwrap()),
        _ => BattleTagExtraction::Conflict,
    }
}
```

Refactor `extract_battle_tag(page_text)` to collect its labeled values and delegate final validation to this function so existing behavior stays covered.

- [ ] **Step 4: Run focused tests and verify GREEN**

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo test --test phase4_login_redesign battletag_ -- --nocapture
```

Expected: all existing and new BattleTag parsing tests pass.

- [ ] **Step 5: Commit Task 1**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/dashen_auth.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs
git commit -m "test: define BattleTag candidate validation"
```

### Task 2: Collect labeled BattleTag candidates from split DOM layouts

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_auth.rs:151-194`
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs:360-397`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs:145-175`

- [ ] **Step 1: Write failing collection-script contract tests**

Extend `collect_script_reads_page_storage_and_resource_urls` with precise requirements:

```rust
assert!(script.contains("battleTagCandidates:"));
assert!(script.contains("createTreeWalker"));
assert!(script.contains("NodeFilter.SHOW_TEXT"));
assert!(script.contains("战网昵称"));
assert!(script.contains("iframe.contentDocument"));
assert!(script.contains("MAX_BATTLETAG_CANDIDATES"));
assert!(!script.contains("outerHTML"));
```

Add a source-level privacy assertion that the returned object contains candidate values and truncation state but no HTML, node paths, or storage contents under the BattleTag key.

- [ ] **Step 2: Run the script test and verify RED**

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo test --test phase4_login_redesign collect_script_reads_page_storage_and_resource_urls -- --nocapture
```

Expected: fails because the script does not yet contain `battleTagCandidates` or DOM traversal.

- [ ] **Step 3: Implement the bounded DOM collector**

Inside `build_collect_candidates_script()`, add a dedicated group and traverse labeled text nodes in document order:

```javascript
const MAX_BATTLETAG_CANDIDATES = 20;
const battleTagGroup = createGroup();
const battleTagPattern = /^\s*[:：]\s*([^\s#]+#[0-9]+)/;

const collectBattleTags = (doc) => {
  if (!doc?.body || battleTagGroup.truncated) return;
  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT);
  const textNodes = [];
  while (walker.nextNode()) textNodes.push(walker.currentNode);
  for (let index = 0; index < textNodes.length; index += 1) {
    const text = String(textNodes[index].nodeValue || '');
    const labelIndex = text.indexOf('战网昵称');
    if (labelIndex < 0) continue;
    let evidence = text.slice(labelIndex + '战网昵称'.length);
    for (let lookahead = 1; lookahead <= 4 && !battleTagPattern.test(evidence); lookahead += 1) {
      const next = textNodes[index + lookahead];
      if (!next) break;
      evidence += ' ' + String(next.nodeValue || '');
    }
    const match = evidence.match(battleTagPattern);
    if (match) add(battleTagGroup, match[1]);
  }
  for (const iframe of doc.querySelectorAll('iframe')) {
    try { collectBattleTags(iframe.contentDocument); } catch (_) { /* inaccessible frame */ }
  }
};
collectBattleTags(document);
```

Use the existing bounded `add` helper with a BattleTag-specific maximum, or parameterize it so exceeding 20 candidates marks `battleTagGroup.truncated = true`. Return:

```javascript
battleTagCandidates: battleTagGroup
```

In `collect_login_identity`, decode the new group with `role_candidate_group` (rename the helper to `candidate_group` if needed) and call:

```rust
extract_battle_tag_candidates(&battle_tag_candidates, battle_tag_truncated)
```

- [ ] **Step 4: Run collection and parsing tests**

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo test --test phase4_login_redesign collect_script -- --nocapture
cargo test --test phase4_login_redesign battletag_ -- --nocapture
```

Expected: both commands pass.

- [ ] **Step 5: Commit Task 2**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/dashen_auth.rs apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs
git commit -m "feat: collect BattleTag from labeled DOM nodes"
```

### Task 3: Pair stable BattleTag with the confirmed same-round credential role

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs:34-152,240-310`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs:255-525`

- [ ] **Step 1: Write failing pairing tests**

Replace the legacy negative auxiliary-evidence expectation with the approved behavior and retain conflict/reset tests:

```rust
#[test]
fn login_capture_pairs_stable_battletag_with_unique_credential_role() {
    let mut identity = LoginCaptureIdentity::default();
    for _ in 0..2 {
        identity.observe(
            Some("101101".to_string()),
            RoleIdExtraction::Missing,
            BattleTagExtraction::Found("Current#1111".to_string()),
        );
    }
    assert_eq!(identity.detected_battle_tag("101101"), Some("Current#1111"));
}

#[test]
fn login_capture_rejects_conflicting_current_role_evidence() {
    let mut identity = LoginCaptureIdentity::default();
    for _ in 0..2 {
        identity.observe(
            Some("101101".to_string()),
            RoleIdExtraction::Conflict,
            BattleTagExtraction::Found("Current#1111".to_string()),
        );
    }
    assert_eq!(identity.detected_battle_tag("101101"), None);
}
```

Keep or add tests proving that credential role changes, candidate missing/conflict, and a new capture identity clear the old candidate.

- [ ] **Step 2: Run pairing tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo test --test phase4_login_redesign login_capture_pairs_stable_battletag_with_unique_credential_role -- --nocapture
```

Expected: fails because `RoleIdExtraction::Missing` currently clears the paired tracker.

- [ ] **Step 3: Implement the approved pairing boundary**

Centralize role selection inside `LoginCaptureIdentity`:

```rust
fn pairing_role_id(
    credential_role_id: Option<&str>,
    current_role: RoleIdExtraction,
) -> Option<String> {
    match current_role {
        RoleIdExtraction::Conflict => None,
        RoleIdExtraction::Found(role_id) => {
            (credential_role_id == Some(role_id.as_str())).then_some(role_id)
        }
        RoleIdExtraction::Missing => credential_role_id.map(str::to_string),
    }
}
```

Use this selected role when observing the BattleTag. Update `confirm_pending_pair` to receive the second observation's `credential_role_id` as well as current-page role evidence so both observations follow the same rule. In `fetch_credentials_from_browser`, pass all three values from the second `collect_login_identity` call.

Do not return a detected BattleTag until `credentials_from_report_result` succeeds and the returned credential role equals the paired role; retain the existing final equality check.

- [ ] **Step 4: Run all login capture tests**

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo test --test phase4_login_redesign -- --nocapture
```

Expected: all tests pass, including role-change, stale-pair, conflict, token mismatch, and privacy cases.

- [ ] **Step 5: Commit Task 3**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs
git commit -m "fix: pair BattleTag with confirmed login role"
```

### Task 4: Browser acceptance, product regression, and full verification

**Files:**
- Verify: `apps/owstats-desktop/src-tauri/src/dashen_auth.rs`
- Verify: `apps/owstats-desktop/src-tauri/src/credentials.rs`
- Verify: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`
- Verify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`

- [ ] **Step 1: Run synthetic browser DOM acceptance**

Use Playwright CLI with a temporary script outside the repository. Construct synthetic pages for:

```html
<div>战网昵称: Example#1234</div>
<div><span>⇄</span><span>战网昵称:</span><span>Example#1234</span><button>切换</button></div>
<div style="display:flex"><span>战网昵称:</span><span>Example#1234</span></div>
<iframe srcdoc="<div>战网昵称: Example#1234</div>"></iframe>
```

Evaluate the generated collection script and assert each case returns exactly one synthetic candidate, with no `pageText`, HTML, token-like key, or node path inside `battleTagCandidates`.

- [ ] **Step 2: Run frontend product-surface tests**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8RealFetch.test.ts
npx tsx scripts/phase6Shell.test.ts
```

Expected: both pass and confirm there is still no manual BattleTag input or arbitrary binding command.

- [ ] **Step 3: Run full frontend verification**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: both commands exit 0; the existing Vite large-chunk warning is acceptable.

- [ ] **Step 4: Run full Rust verification without disturbing a running desktop app**

```powershell
cd apps\owstats-desktop\src-tauri
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-battletag-target'
cargo fmt --check
cargo test --tests
```

Expected: formatting passes; all non-live tests pass; the real-credential live test remains ignored by design.

- [ ] **Step 5: Run privacy and workspace hygiene checks**

```powershell
git diff --check
rg -n -i "customerToken|authorization|OW_DASHEN_TOKEN" apps/owstats-desktop/src-tauri/src/dashen_auth.rs apps/owstats-desktop/src-tauri/src/credentials.rs
git status --short
```

Expected: no new sensitive output path; only intended implementation files plus the pre-existing unrelated `apps/owstats-desktop/src-tauri/Cargo.toml` modification appear before implementation commits, and that file is never staged.

- [ ] **Step 6: Commit any verification-only corrections**

If verification requires a correction, first add a focused failing regression test, then make the minimal fix and commit only the intended files:

```powershell
git add -- apps/owstats-desktop/src-tauri/src/dashen_auth.rs apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs apps/owstats-desktop/scripts/phase8RealFetch.test.ts
git commit -m "test: verify reliable BattleTag login capture"
```
