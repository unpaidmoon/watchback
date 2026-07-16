import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountManager, AccountRemovalDialog } from "../src/features/command-center/AccountManager";

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const commandCenterStyles = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");
const tauriLibSource = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

assert.match(onboardingSource, /listAccounts\(\)/);
assert.match(onboardingSource, /<AccountManager/);
assert.match(onboardingSource, /window\.location\.reload\(\)/);
assert.match(onboardingSource, /dashboardCache = null/);
assert.match(onboardingSource, /添加账号后即可同步和查看本地数据/);
assert.ok(
  onboardingSource.indexOf("const nextAccountStatus = await listAccounts()") <
    onboardingSource.indexOf("const [nextStatus, nextProfile, nextTargetStatus] = await Promise.all"),
  "账户列表必须先于账户域命令加载",
);
assert.match(
  onboardingSource,
  /if \(!nextAccountStatus\.activeAccountId\) \{[\s\S]*?status: null,[\s\S]*?profile: null,[\s\S]*?targetStatus: null,[\s\S]*?return;/,
  "没有活动账号时不得继续调用账户域命令",
);
assert.match(
  onboardingSource,
  /dashboardCache\.accountStatus\.activeAccountId === nextAccountStatus\.activeAccountId/,
  "仪表盘缓存必须绑定活动账号",
);
assert.match(onboardingSource, /async function runSwitchAccount[\s\S]*?dashboardCache = null;[\s\S]*?window\.location\.reload\(\)/);
assert.match(onboardingSource, /result\.removed && result\.reloadRequired[\s\S]*?dashboardCache = null;[\s\S]*?window\.location\.reload\(\)/);
assert.doesNotMatch(onboardingSource, /runLogout/);
assert.doesNotMatch(onboardingSource, />退出登录</);
assert.doesNotMatch(tauriLibSource, /commands::logout\b/);
assert.doesNotMatch(tauriLibSource, /commands::start_dashen_login\b/);
assert.match(tauriLibSource, /commands::cancel_dashen_login\b/);
assert.match(commandCenterStyles, /\.phase6CommandCenter \.accountManagerPanel\s*\{[\s\S]*?background:\s*var\(--glass-secondary\);[\s\S]*?box-shadow:\s*none;/);
assert.match(commandCenterStyles, /@media \(max-width: 900px\)[\s\S]*?\.phase6CommandCenter \.accountManagerRow[\s\S]*?grid-template-columns:\s*1fr/);

const html = renderToStaticMarkup(
  createElement(AccountManager, {
    accounts: [
      {
        accountId: "account-a",
        battleTag: "Tester#5479",
        isActive: true,
        credentialState: "ready",
        createdAt: "",
        lastUsedAt: "2026-07-16T00:00:00Z",
      },
      {
        accountId: "account-b",
        battleTag: "Second#2222",
        isActive: false,
        credentialState: "stale",
        createdAt: "",
        lastUsedAt: "2026-07-15T00:00:00Z",
      },
    ],
    busyAction: null,
    onAdd: () => undefined,
    onSwitch: () => undefined,
    onRelogin: () => undefined,
    onRemove: () => undefined,
  }),
);

assert.match(html, /添加新的网易大神账号/);
assert.match(html, /Tester#5479/);
assert.match(html, /Second#2222/);
assert.equal((html.match(/重新登录/g) ?? []).length, 2);
assert.equal((html.match(/退出账号/g) ?? []).length, 2);
assert.match(html, /disabled=""[^>]*>当前账号</);
assert.match(html, />切换账号</);
assert.doesNotMatch(html, /roleId|token|customerToken/);
assert.match(html, /<h2[^>]*tabindex="-1"[^>]*>账号管理<\/h2>/);

const accountActionGroups = [...html.matchAll(/<div class="accountManagerActions" role="group"[^>]*>([\s\S]*?)<\/div>/g)];
assert.equal(accountActionGroups.length, 2, "每个账号行都提供一个具名操作组");
for (const group of accountActionGroups) {
  assert.equal((group[1]?.match(/<button/g) ?? []).length, 3, "每个账号行始终保留三个操作位置");
  assert.match(group[0], /aria-label="[^"]+ 的账号操作"/);
}

const removalDialog = renderToStaticMarkup(
  createElement(AccountRemovalDialog, {
    account: {
      accountId: "account-a",
      battleTag: "Tester#5479",
      isActive: true,
      credentialState: "ready",
      createdAt: "",
      lastUsedAt: "2026-07-16T00:00:00Z",
    },
    busy: false,
    fallbackFocusRef: { current: null },
    onCancel: () => undefined,
    onConfirm: () => undefined,
  }),
);

assert.match(removalDialog, /role="alertdialog"/);
assert.match(removalDialog, /aria-modal="true"/);
assert.match(removalDialog, /aria-labelledby="account-removal-title"/);
assert.match(removalDialog, /aria-describedby="account-removal-description"/);
assert.match(removalDialog, /Tester#5479/);
assert.match(removalDialog, /永久删除该账号保存的凭证信息和对应本地数据库/);
assert.match(removalDialog, />取消</);
assert.match(removalDialog, />永久退出账号</);
assert.equal((removalDialog.match(/<button/g) ?? []).length, 2, "确认框只有取消和永久退出两个清晰动作");
