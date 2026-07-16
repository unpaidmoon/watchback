import type {
  AccountLoginResult,
  AccountManagerStatus,
  AccountSummary,
  LoginTargetStatus,
  OnboardingStatus,
  PlayerProfile,
  RemoveAccountResult,
} from "../../shared/types";

export type DashboardSnapshot = {
  accountStatus: AccountManagerStatus;
  status: OnboardingStatus | null;
  profile: PlayerProfile | null;
  targetStatus: LoginTargetStatus | null;
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
      snapshot: { accountStatus, status: null, profile: null, targetStatus: null },
    };
  }

  const scopedDashboard = await options.loadAccountScope();
  if (!options.isCurrent(options.requestGeneration)) {
    return { kind: "stale" };
  }
  return { kind: "ready", snapshot: { accountStatus, ...scopedDashboard } };
}

export type BusyAccountAction = {
  kind: "add" | "switch" | "relogin" | "remove";
  accountId?: string;
};

type OperationLifecycle = {
  begin: () => boolean;
  release: () => void;
  setBusy: (action: BusyAccountAction | null) => void;
  setNotice: (message: string | null) => void;
  setError: (message: string | null) => void;
  formatError: (error: unknown) => string;
  refreshAccounts: () => Promise<void>;
  clearCache: () => void;
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

export function runReloginAccountOperation(account: AccountSummary, dependencies: LoginOperationDependencies) {
  return runOperation(
    { kind: "relogin", accountId: account.accountId },
    dependencies,
    "重新登录失败",
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

export function runSwitchAccountOperation(account: AccountSummary, dependencies: SwitchOperationDependencies) {
  return runOperation(
    { kind: "switch", accountId: account.accountId },
    dependencies,
    "切换账号失败",
    async () => {
      await dependencies.invoke();
      dependencies.clearCache();
      dependencies.reload();
    },
  );
}

type RemoveOperationDependencies = OperationLifecycle & {
  invoke: () => Promise<RemoveAccountResult>;
  reload: () => void;
};

export function runRemoveAccountOperation(account: AccountSummary, dependencies: RemoveOperationDependencies) {
  return runOperation(
    { kind: "remove", accountId: account.accountId },
    dependencies,
    "退出账号失败",
    async () => {
      const result = await dependencies.invoke();
      if (result.removed && result.reloadRequired) {
        dependencies.clearCache();
        dependencies.reload();
        return;
      }
      await dependencies.refreshAccounts();
      dependencies.setNotice(result.message);
    },
  );
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
