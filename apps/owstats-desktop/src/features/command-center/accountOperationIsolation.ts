import type {
  AccountLoginResult,
  AccountManagerStatus,
  AccountSummary,
  OnboardingStatus,
  PlayerProfile,
  RemoveAccountResult,
} from "../../shared/types";

export type DashboardSnapshot = {
  accountStatus: AccountManagerStatus;
  status: OnboardingStatus | null;
  profile: PlayerProfile | null;
};

type AccountScopedDashboard = Omit<DashboardSnapshot, "accountStatus">;

type DashboardRequestOptions = {
  requestGeneration: number;
  isCurrent: (requestGeneration: number) => boolean;
  cachedSnapshot: DashboardSnapshot | null;
  listAccounts: () => Promise<AccountManagerStatus>;
  loadAccountScope: () => Promise<AccountScopedDashboard>;
  onAccountStatus: (status: AccountManagerStatus, reusableCache: DashboardSnapshot | null) => void;
};

export type DashboardRequestResult =
  | { kind: "stale" }
  | { kind: "ready"; snapshot: DashboardSnapshot };

export async function loadDashboardRequest(options: DashboardRequestOptions): Promise<DashboardRequestResult> {
  const accountStatus = await options.listAccounts();
  if (!options.isCurrent(options.requestGeneration)) {
    return { kind: "stale" };
  }

  const reusableCache = options.cachedSnapshot?.accountStatus.activeAccountId === accountStatus.activeAccountId
    ? { ...options.cachedSnapshot, accountStatus }
    : null;
  options.onAccountStatus(accountStatus, reusableCache);

  if (!accountStatus.activeAccountId) {
    return {
      kind: "ready",
      snapshot: { accountStatus, status: null, profile: null },
    };
  }

  const scopedDashboard = await options.loadAccountScope();
  if (!options.isCurrent(options.requestGeneration)) {
    return { kind: "stale" };
  }
  return { kind: "ready", snapshot: { accountStatus, ...scopedDashboard } };
}

export type BusyAccountAction = {
  kind: "add" | "switch" | "remove";
  accountId?: string;
};

type ExistingLoginFallbackDependencies = {
  settle: () => Promise<{ released: boolean }>;
  addAccount: () => Promise<void>;
};

export async function runExistingLoginFallback(dependencies: ExistingLoginFallbackDependencies) {
  const settled = await dependencies.settle();
  if (!settled.released) {
    throw new Error("二维码登录尚未释放账号操作，请重试。");
  }
  await dependencies.addAccount();
}

type OperationLifecycle = {
  begin: () => boolean;
  release: () => void;
  setBusy: (action: BusyAccountAction | null) => void;
  setNotice: (message: string | null) => void;
  setError: (message: string | null) => void;
  formatError: (error: unknown) => string;
  refreshAccounts: () => Promise<void>;
  clearCache: () => void;
  failClosed: () => void;
  refreshAfterMutation: () => Promise<void>;
};

async function runOperation(
  action: BusyAccountAction,
  lifecycle: OperationLifecycle,
  failureLabel: string,
  work: () => Promise<void>,
  refreshAfterFailure = false,
) {
  if (!lifecycle.begin()) {
    return false;
  }
  lifecycle.setBusy(action);
  lifecycle.setNotice(null);
  lifecycle.setError(null);
  try {
    await work();
    return true;
  } catch (error) {
    if (refreshAfterFailure) {
      try {
        await lifecycle.refreshAccounts();
      } catch {
        // Preserve the original operation failure as the user-facing recovery message.
      }
    }
    lifecycle.setError(`${failureLabel}：${lifecycle.formatError(error)}。请重试。`);
    return false;
  } finally {
    lifecycle.setBusy(null);
    lifecycle.release();
  }
}

type LoginOperationDependencies = OperationLifecycle & {
  invoke: () => Promise<AccountLoginResult>;
};

export function runAddAccountOperation(dependencies: LoginOperationDependencies) {
  return runOperation(
    { kind: "add" },
    dependencies,
    "添加账号失败",
    async () => {
      const result = await dependencies.invoke();
      await dependencies.refreshAccounts();
      dependencies.setNotice(result.message);
    },
    true,
  );
}

type SwitchOperationDependencies = OperationLifecycle & {
  invoke: () => Promise<AccountManagerStatus>;
  reload: () => void;
};

async function runContextMutation<T extends { message: string }>(
  action: BusyAccountAction,
  dependencies: OperationLifecycle & { invoke: () => Promise<T>; reload: () => void },
  mutationFailureLabel: string,
  refreshFailureMessage: string,
) {
  if (!dependencies.begin()) {
    return false;
  }
  dependencies.setBusy(action);
  dependencies.setNotice(null);
  dependencies.setError(null);
  try {
    let result: T;
    try {
      result = await dependencies.invoke();
    } catch (error) {
      dependencies.setError(`${mutationFailureLabel}：${dependencies.formatError(error)}。请重试。`);
      return false;
    }
    dependencies.clearCache();
    dependencies.failClosed();
    try {
      dependencies.reload();
    } catch {
      // Reload is only a best-effort takeover. The in-process refresh below is mandatory.
    }
    try {
      await dependencies.refreshAfterMutation();
      dependencies.setNotice(result.message);
    } catch {
      dependencies.setError(refreshFailureMessage);
    }
    return true;
  } finally {
    dependencies.setBusy(null);
    dependencies.release();
  }
}

export function runSwitchAccountOperation(account: AccountSummary, dependencies: SwitchOperationDependencies) {
  return runContextMutation(
    { kind: "switch", accountId: account.accountId },
    dependencies,
    "切换账号失败",
    "账号已切换，但重新加载失败，请重试或重启。",
  );
}

type RemoveOperationDependencies = OperationLifecycle & {
  invoke: () => Promise<RemoveAccountResult>;
  reload: () => void;
};

export function runRemoveAccountOperation(account: AccountSummary, dependencies: RemoveOperationDependencies) {
  const action = { kind: "remove", accountId: account.accountId } as const;
  if (!dependencies.begin()) {
    return Promise.resolve(false);
  }
  dependencies.setBusy(action);
  dependencies.setNotice(null);
  dependencies.setError(null);
  return (async () => {
    try {
      const result = await dependencies.invoke();
      if (result.removed && result.reloadRequired) {
        dependencies.clearCache();
        dependencies.failClosed();
        try {
          dependencies.reload();
        } catch {
          // Reload is best-effort; the fresh account-scoped request is authoritative.
        }
        try {
          await dependencies.refreshAfterMutation();
          dependencies.setNotice(result.message);
        } catch {
          dependencies.setError("账号已退出，但重新加载失败，请重试或重启。");
        }
        return true;
      }
      await dependencies.refreshAccounts();
      dependencies.setNotice(result.message);
      return true;
    } catch (error) {
      dependencies.setError(`退出账号失败：${dependencies.formatError(error)}。请重试。`);
      return false;
    } finally {
      dependencies.setBusy(null);
      dependencies.release();
    }
  })();
}

export function applyRemovalDecision(
  decision: "cancel" | "confirm",
  account: AccountSummary,
  remove: (account: AccountSummary) => void,
) {
  if (decision === "confirm") {
    remove(account);
  }
}
