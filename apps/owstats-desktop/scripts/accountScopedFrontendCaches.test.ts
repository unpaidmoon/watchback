import assert from "node:assert/strict";
import { clearAccountScopedFrontendCaches } from "../src/shared/accountScopedFrontendCaches";
import { getMatchesPageCache, updateMatchesPageCache } from "../src/shared/matchesPageCache";
import { requestPersonalSeasonReviewOnce } from "../src/pages/PersonalReviewPage";
import { runRemoveAccountOperation, runSwitchAccountOperation } from "../src/features/command-center/accountOperationIsolation";
import type { AccountManagerStatus, AccountSummary, PersonalSeasonReview } from "../src/shared/types";

updateMatchesPageCache({
  filters: { season: 21 },
  loadedFilterKey: "season:21",
  hasLoaded: true,
  matches: [],
});

let resolveOldReview!: (review: PersonalSeasonReview) => void;
const oldReviewPromise = requestPersonalSeasonReviewOnce(21, () => new Promise((resolve) => {
  resolveOldReview = resolve;
}));

clearAccountScopedFrontendCaches();

assert.equal(getMatchesPageCache(), null, "账号上下文变化必须清空对局页模块缓存");

let freshReviewLoads = 0;
const freshReview = {} as PersonalSeasonReview;
const freshReviewPromise = requestPersonalSeasonReviewOnce(21, async () => {
  freshReviewLoads += 1;
  return freshReview;
});
assert.notEqual(freshReviewPromise, oldReviewPromise, "账号上下文变化后不得复用旧账号的战报请求");
assert.equal(await freshReviewPromise, freshReview);
assert.equal(freshReviewLoads, 1);

resolveOldReview({} as PersonalSeasonReview);
await oldReviewPromise;

const firstAccount: AccountSummary = {
  accountId: "synthetic-account-a",
  battleTag: "SyntheticA#1111",
  isActive: true,
  credentialState: "ready",
  createdAt: "2026-07-16T00:00:00Z",
  lastUsedAt: "2026-07-16T00:00:00Z",
};
const secondAccount: AccountSummary = {
  ...firstAccount,
  accountId: "synthetic-account-b",
  battleTag: "SyntheticB#2222",
  isActive: false,
};
const secondStatus: AccountManagerStatus = {
  activeAccountId: secondAccount.accountId,
  accounts: [{ ...firstAccount, isActive: false }, { ...secondAccount, isActive: true }],
  message: "已切换合成账号",
};

function seedOldAccountCaches(season: number) {
  updateMatchesPageCache({
    filters: { season },
    loadedFilterKey: `season:${season}`,
    hasLoaded: true,
    matches: [],
  });
  let resolveReview!: (review: PersonalSeasonReview) => void;
  const reviewPromise = requestPersonalSeasonReviewOnce(season, () => new Promise((resolve) => {
    resolveReview = resolve;
  }));
  return { reviewPromise, resolveReview };
}

function mutationLifecycle(refreshAfterMutation: () => Promise<void>) {
  let failClosedCount = 0;
  return {
    dependencies: {
      begin: () => true,
      release: () => undefined,
      setBusy: () => undefined,
      setNotice: () => undefined,
      setError: () => undefined,
      formatError: () => "合成错误",
      refreshAccounts: async () => undefined,
      clearCache: clearAccountScopedFrontendCaches,
      failClosed: () => { failClosedCount += 1; },
      refreshAfterMutation,
    },
    failClosedCount: () => failClosedCount,
  };
}

for (const reloadThrows of [false, true]) {
  const oldCaches = seedOldAccountCaches(22);
  const lifecycle = mutationLifecycle(async () => undefined);
  let freshReviewPromise: Promise<PersonalSeasonReview> | null = null;
  await runSwitchAccountOperation(secondAccount, {
    ...lifecycle.dependencies,
    invoke: async () => secondStatus,
    reload: () => {
      assert.equal(getMatchesPageCache(), null, "switch reload 前不能保留旧账号对局快照");
      freshReviewPromise = requestPersonalSeasonReviewOnce(22, async () => ({} as PersonalSeasonReview));
      assert.notEqual(freshReviewPromise, oldCaches.reviewPromise, "switch reload 前不能复用旧账号战报请求");
      if (reloadThrows) throw new Error("reload unavailable");
    },
  });
  assert.equal(lifecycle.failClosedCount(), 1, "reload no-op 或抛错时仍必须 fail-close");
  await freshReviewPromise;
  oldCaches.resolveReview({} as PersonalSeasonReview);
  await oldCaches.reviewPromise;
}

{
  const oldCaches = seedOldAccountCaches(23);
  const lifecycle = mutationLifecycle(async () => { throw new Error("refresh failed"); });
  await runRemoveAccountOperation(firstAccount, {
    ...lifecycle.dependencies,
    invoke: async () => ({ removed: true, activeAccountId: null, message: "已退出合成账号", reloadRequired: true }),
    reload: () => {
      assert.equal(getMatchesPageCache(), null, "remove reload 前不能保留旧账号对局快照");
      assert.notEqual(
        requestPersonalSeasonReviewOnce(23, async () => ({} as PersonalSeasonReview)),
        oldCaches.reviewPromise,
        "remove reload 前不能复用旧账号战报请求",
      );
    },
  });
  assert.equal(lifecycle.failClosedCount(), 1, "remove 后刷新失败仍必须 fail-close");
  oldCaches.resolveReview({} as PersonalSeasonReview);
  await oldCaches.reviewPromise;
}

{
  const oldCaches = seedOldAccountCaches(21);
  const lifecycle = mutationLifecycle(async () => undefined);
  await runSwitchAccountOperation(secondAccount, {
    ...lifecycle.dependencies,
    invoke: async () => { throw new Error("mutation failed"); },
    reload: () => undefined,
  });
  assert.notEqual(getMatchesPageCache(), null, "mutation 失败不得误清旧账号对局缓存");
  assert.equal(
    requestPersonalSeasonReviewOnce(21, async () => ({} as PersonalSeasonReview)),
    oldCaches.reviewPromise,
    "mutation 失败不得误清仍属当前账号的战报请求",
  );
  assert.equal(lifecycle.failClosedCount(), 0, "mutation 失败不得误置账号上下文失效");
  oldCaches.resolveReview({} as PersonalSeasonReview);
  await oldCaches.reviewPromise;
  clearAccountScopedFrontendCaches();
}

console.log("account-scoped frontend cache tests passed");
