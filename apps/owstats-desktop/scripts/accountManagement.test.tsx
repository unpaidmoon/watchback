import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountManager, AccountRemovalDialog } from "../src/features/command-center/AccountManager";
import { loadDashboardRequest } from "../src/features/command-center/accountOperationIsolation";
import type { AccountManagerStatus } from "../src/shared/types";

const emptyAccountStatus: AccountManagerStatus = {
  activeAccountId: null,
  accounts: [],
  message: "尚未添加账号",
};
let scopedLoadCount = 0;
const emptyDashboard = await loadDashboardRequest({
  requestGeneration: 1,
  isCurrent: () => true,
  cachedSnapshot: null,
  listAccounts: async () => emptyAccountStatus,
  loadAccountScope: async () => {
    scopedLoadCount += 1;
    return { status: null, profile: null };
  },
  onAccountStatus: () => undefined,
});
assert.deepEqual(emptyDashboard, {
  kind: "ready",
  snapshot: { accountStatus: emptyAccountStatus, status: null, profile: null },
});
assert.equal(scopedLoadCount, 0, "没有活动账号时不得读取任何账号域数据");

const activeAccountStatus: AccountManagerStatus = {
  activeAccountId: "account-b",
  accounts: [],
  message: "已切换账号",
};
let reusableCache: unknown = "not-called";
await loadDashboardRequest({
  requestGeneration: 2,
  isCurrent: () => true,
  cachedSnapshot: { accountStatus: { ...activeAccountStatus, activeAccountId: "account-a" }, status: null, profile: null },
  listAccounts: async () => activeAccountStatus,
  loadAccountScope: async () => ({ status: null, profile: null }),
  onAccountStatus: (_status, cache) => { reusableCache = cache; },
});
assert.equal(reusableCache, null, "活动账号改变后不得复用旧账号缓存");

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
    onRemove: () => undefined,
  }),
);

assert.ok(html.includes("添加或更新网易大神账号"));
assert.ok(html.includes("Tester#5479"));
assert.ok(html.includes("Second#2222"));
assert.doesNotMatch(html, /重新登录|登录中/);
assert.equal((html.match(/退出账号/g) ?? []).length, 2);
assert.match(html, /disabled=""[^>]*>当前账号</);
assert.ok(html.includes(">切换账号<"));
assert.doesNotMatch(html, /roleId|token|customerToken/);
assert.match(html, /<h2[^>]*tabindex="-1"[^>]*>账号管理<\/h2>/);
assert.equal((html.match(/accountManagerAddRow/g) ?? []).length, 1, "添加账号保持为独立一行");

const accountActionGroups = [...html.matchAll(/<div class="accountManagerActions" role="group"[^>]*>([\s\S]*?)<\/div>/g)];
assert.equal(accountActionGroups.length, 2, "每个账号行都提供一个具名操作组");
for (const group of accountActionGroups) {
  assert.equal((group[1]?.match(/<button/g) ?? []).length, 2, "每个账号行只保留切换和退出操作");
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

assert.ok(removalDialog.includes('role="alertdialog"'));
assert.ok(removalDialog.includes('aria-modal="true"'));
assert.ok(removalDialog.includes('aria-labelledby="account-removal-title"'));
assert.ok(removalDialog.includes('aria-describedby="account-removal-description"'));
assert.ok(removalDialog.includes("Tester#5479"));
assert.ok(removalDialog.includes("永久删除该账号保存的凭证信息和对应本地数据库"));
assert.doesNotMatch(removalDialog, /Second#2222|共享设置|浏览器|账号列表/);
assert.ok(removalDialog.includes(">取消<"));
assert.ok(removalDialog.includes(">永久退出账号<"));
assert.equal((removalDialog.match(/<button/g) ?? []).length, 2, "确认框只有取消和永久退出两个清晰动作");
