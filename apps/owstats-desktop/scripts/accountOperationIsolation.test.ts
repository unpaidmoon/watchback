import assert from "node:assert/strict";
import {
  applyRemovalDecision,
  loadDashboardRequest,
  runAddAccountOperation,
  runReloginAccountOperation,
  runRemoveAccountOperation,
  runSwitchAccountOperation,
} from "../src/features/command-center/accountOperationIsolation";
import type { AccountManagerStatus, AccountSummary } from "../src/shared/types";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

const firstAccount: AccountSummary = {
  accountId: "account-a",
  battleTag: "Tester#1111",
  isActive: true,
  credentialState: "ready",
  createdAt: "2026-07-16T00:00:00Z",
  lastUsedAt: "2026-07-16T00:00:00Z",
};
const secondAccount: AccountSummary = { ...firstAccount, accountId: "account-b", battleTag: "Second#2222" };
const firstStatus: AccountManagerStatus = { activeAccountId: firstAccount.accountId, accounts: [firstAccount], message: "已加载账号" };
const secondStatus: AccountManagerStatus = { activeAccountId: secondAccount.accountId, accounts: [secondAccount], message: "已加载账号" };

{
  const accounts = deferred<AccountManagerStatus>();
  let generation = 1;
  let accountUpdates = 0;
  let scopedLoads = 0;
  const request = loadDashboardRequest({
    requestGeneration: 1,
    isCurrent: (requestGeneration) => requestGeneration === generation,
    cachedSnapshot: null,
    listAccounts: () => accounts.promise,
    loadAccountScope: async () => {
      scopedLoads += 1;
      return { status: null, profile: null, targetStatus: null };
    },
    onAccountStatus: () => {
      accountUpdates += 1;
    },
  });
  generation = 2;
  accounts.resolve(firstStatus);
  assert.equal((await request).kind, "stale");
  assert.equal(accountUpdates, 0, "陈旧账号请求不得覆盖新 generation");
  assert.equal(scopedLoads, 0, "陈旧账号请求不得继续加载账号域数据");
}

{
  const scoped = deferred<{ status: null; profile: null; targetStatus: null }>();
  let reusableCache: unknown = "not-called";
  const resultPromise = loadDashboardRequest({
    requestGeneration: 3,
    isCurrent: () => true,
    cachedSnapshot: { accountStatus: firstStatus, status: null, profile: null, targetStatus: null },
    listAccounts: async () => secondStatus,
    loadAccountScope: () => scoped.promise,
    onAccountStatus: (_status, cache) => {
      reusableCache = cache;
    },
  });
  scoped.resolve({ status: null, profile: null, targetStatus: null });
  assert.equal((await resultPromise).kind, "ready");
  assert.equal(reusableCache, null, "activeAccountId 不匹配时必须丢弃缓存");
}

{
  const scoped = deferred<{ status: null; profile: null; targetStatus: null }>();
  let generation = 4;
  let appliedAccountId = secondAccount.accountId;
  const oldRequest = loadDashboardRequest({
    requestGeneration: 4,
    isCurrent: (requestGeneration) => requestGeneration === generation,
    cachedSnapshot: null,
    listAccounts: async () => firstStatus,
    loadAccountScope: () => scoped.promise,
    onAccountStatus: () => undefined,
  });
  await Promise.resolve();
  generation = 5;
  scoped.resolve({ status: null, profile: null, targetStatus: null });
  const result = await oldRequest;
  if (result.kind === "ready") appliedAccountId = result.snapshot.accountStatus.activeAccountId ?? "";
  assert.equal(result.kind, "stale");
  assert.equal(appliedAccountId, secondAccount.accountId, "旧 scoped 响应不得覆盖新活动账号");
}

function operationHarness() {
  let locked = false;
  let busy: unknown = null;
  let releaseCount = 0;
  let notice: string | null = null;
  let error: string | null = null;
  let refreshCount = 0;
  let clearCount = 0;
  return {
    deps: {
      begin: () => {
        if (locked) return false;
        locked = true;
        return true;
      },
      release: () => {
        locked = false;
        releaseCount += 1;
      },
      setBusy: (value: unknown) => {
        busy = value;
      },
      setNotice: (value: string | null) => {
        notice = value;
      },
      setError: (value: string | null) => {
        error = value;
      },
      formatError: () => "安全错误",
      refreshAccounts: async () => {
        refreshCount += 1;
      },
      clearCache: () => {
        clearCount += 1;
      },
    },
    state: () => ({ locked, busy, releaseCount, notice, error, refreshCount, clearCount }),
  };
}

for (const reload of [() => undefined, () => { throw new Error("reload unavailable"); }]) {
  const harness = operationHarness();
  await runSwitchAccountOperation(secondAccount, { ...harness.deps, invoke: async () => secondStatus, reload });
  assert.equal(harness.state().locked, false, "switch reload 返回或抛错后都必须释放锁");
  assert.equal(harness.state().busy, null, "switch reload 返回或抛错后都必须清除 busy");
  assert.equal(harness.state().releaseCount, 1, "switch 生命周期只释放一次");
}

{
  const harness = operationHarness();
  await runSwitchAccountOperation(secondAccount, { ...harness.deps, invoke: async () => { throw new Error("switch failed"); }, reload: () => undefined });
  assert.equal(harness.state().locked, false);
  assert.equal(harness.state().busy, null);
  assert.equal(harness.state().error, "切换账号失败：安全错误。请重试。");
}

{
  let removals = 0;
  applyRemovalDecision("cancel", firstAccount, () => { removals += 1; });
  assert.equal(removals, 0, "取消退出不得调用 remove");
  applyRemovalDecision("confirm", firstAccount, () => { removals += 1; });
  assert.equal(removals, 1, "确认退出必须调用 remove");
}

for (const reload of [() => undefined, () => { throw new Error("reload unavailable"); }]) {
  const harness = operationHarness();
  await runRemoveAccountOperation(firstAccount, {
    ...harness.deps,
    invoke: async () => ({ removed: true, activeAccountId: null, message: "账号已退出", reloadRequired: true }),
    reload,
  });
  assert.equal(harness.state().locked, false, "remove reload 返回或抛错后都必须释放锁");
  assert.equal(harness.state().busy, null, "remove reload 返回或抛错后都必须清除 busy");
  assert.equal(harness.state().releaseCount, 1, "remove 生命周期只释放一次");
  assert.equal(harness.state().clearCount, 1, "remove 必须在 reload 前清理缓存");
}

{
  const harness = operationHarness();
  await runRemoveAccountOperation(secondAccount, {
    ...harness.deps,
    invoke: async () => ({ removed: false, activeAccountId: secondAccount.accountId, message: "账号未退出", reloadRequired: false }),
    reload: () => undefined,
  });
  assert.equal(harness.state().locked, false);
  assert.equal(harness.state().releaseCount, 1);
  assert.equal(harness.state().refreshCount, 1);
  assert.equal(harness.state().notice, "账号未退出");
}

{
  const harness = operationHarness();
  await runRemoveAccountOperation(firstAccount, {
    ...harness.deps,
    invoke: async () => { throw new Error("remove failed"); },
    reload: () => undefined,
  });
  assert.equal(harness.state().locked, false);
  assert.equal(harness.state().busy, null);
  assert.equal(harness.state().releaseCount, 1);
  assert.equal(harness.state().error, "退出账号失败：安全错误。请重试。");
}

for (const run of [
  (harness: ReturnType<typeof operationHarness>) => runAddAccountOperation({
    ...harness.deps,
    invoke: async () => ({ kind: "differentExistingAccount", accountId: secondAccount.accountId, battleTag: secondAccount.battleTag, message: "检测到其他账号，请选择是否切换", shouldOfferSwitch: true }),
  }),
  (harness: ReturnType<typeof operationHarness>) => runReloginAccountOperation(firstAccount, {
    ...harness.deps,
    invoke: async () => ({ kind: "differentNewAccount", accountId: null, battleTag: secondAccount.battleTag, message: "登录账号不一致，当前账号未更改", shouldOfferSwitch: false }),
  }),
]) {
  const harness = operationHarness();
  await run(harness);
  assert.equal(harness.state().locked, false);
  assert.equal(harness.state().busy, null);
  assert.equal(harness.state().refreshCount, 1, "mismatch 结果仍需刷新账号列表");
  assert.match(harness.state().notice ?? "", /账号/, "只显示后端返回的安全中文消息");
  assert.equal(harness.state().clearCount, 0, "mismatch 不得自动切换或清理缓存");
}
