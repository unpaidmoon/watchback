import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountManager, AccountRemovalDialog } from "../src/features/command-center/AccountManager";

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
    onCancel: () => undefined,
    onConfirm: () => undefined,
  }),
);

assert.match(removalDialog, /role="alertdialog"/);
assert.match(removalDialog, /aria-modal="true"/);
assert.match(removalDialog, /Tester#5479/);
assert.match(removalDialog, /永久删除该账号保存的凭证信息和对应本地数据库/);
assert.match(removalDialog, />取消</);
assert.match(removalDialog, />永久退出账号</);
