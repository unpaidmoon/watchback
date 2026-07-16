import { useEffect, useRef, useState } from "react";
import type { AccountSummary } from "../../shared/types";

export type AccountManagerProps = {
  accounts: AccountSummary[];
  busyAction: { kind: "add" | "switch" | "relogin" | "remove"; accountId?: string } | null;
  onAdd: () => void;
  onSwitch: (account: AccountSummary) => void;
  onRelogin: (account: AccountSummary) => void;
  onRemove: (account: AccountSummary) => void;
};

type AccountRemovalDialogProps = {
  account: AccountSummary;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
};

export function AccountRemovalDialog({ account, busy, onCancel, onConfirm }: AccountRemovalDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    cancelButtonRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        onCancel();
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const buttons = [cancelButtonRef.current, confirmButtonRef.current].filter(
        (button): button is HTMLButtonElement => button !== null && !button.disabled,
      );
      if (buttons.length === 0) {
        event.preventDefault();
        return;
      }

      const currentIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const nextIndex = event.shiftKey
        ? (currentIndex <= 0 ? buttons.length : currentIndex) - 1
        : (currentIndex + 1) % buttons.length;
      event.preventDefault();
      buttons[nextIndex]?.focus();
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [busy, onCancel]);

  return (
    <div className="accountRemovalBackdrop" role="presentation" onClick={busy ? undefined : onCancel}>
      <section
        className="accountRemovalDialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="account-removal-title"
        aria-describedby="account-removal-description"
        onClick={(event) => event.stopPropagation()}
      >
        <h3 id="account-removal-title">永久退出账号</h3>
        <p id="account-removal-description">
          确认退出 {account.battleTag}？这会永久删除该账号保存的凭证信息和对应本地数据库，无法撤销。
        </p>
        <div className="accountRemovalActions">
          <button ref={cancelButtonRef} type="button" onClick={onCancel} disabled={busy}>
            取消
          </button>
          <button ref={confirmButtonRef} className="dangerButton" type="button" onClick={onConfirm} disabled={busy}>
            永久退出账号
          </button>
        </div>
      </section>
    </div>
  );
}

function credentialLabel(state: AccountSummary["credentialState"]) {
  switch (state) {
    case "ready":
      return "登录有效";
    case "stale":
      return "登录已失效";
    case "incomplete":
      return "登录信息不完整";
    case "missing":
      return "尚未登录";
    default:
      return "登录状态未知";
  }
}

export function AccountManager({
  accounts,
  busyAction,
  onAdd,
  onSwitch,
  onRelogin,
  onRemove,
}: AccountManagerProps) {
  const [pendingRemoval, setPendingRemoval] = useState<AccountSummary | null>(null);
  const isBusy = busyAction !== null;

  function confirmRemoval() {
    if (!pendingRemoval) {
      return;
    }

    onRemove(pendingRemoval);
    setPendingRemoval(null);
  }

  return (
    <section className="accountManager" aria-labelledby="account-manager-title">
      <div className="accountManagerHeading">
        <h2 id="account-manager-title">账号管理</h2>
        <p>每个账号的凭证和本地数据独立保存。</p>
      </div>

      <div className="accountManagerRows">
        <div className="accountManagerRow accountManagerAddRow">
          <div>
            <span className="accountManagerAccountName">添加新的网易大神账号</span>
            <span className="accountManagerAccountMeta">登录另一个账号并创建独立数据空间</span>
          </div>
          <button className="primaryButton" type="button" onClick={onAdd} disabled={isBusy}>
            {busyAction?.kind === "add" ? "正在添加…" : "添加账号"}
          </button>
        </div>

        {accounts.map((account) => {
          const isThisAccountBusy = busyAction?.accountId === account.accountId;

          return (
            <div className="accountManagerRow" key={account.accountId}>
              <div>
                <span className="accountManagerAccountName">{account.battleTag}</span>
                <span className="accountManagerAccountMeta">{credentialLabel(account.credentialState)}</span>
              </div>
              <div className="accountManagerActions" aria-label={`${account.battleTag} 的账号操作`}>
                {account.isActive ? (
                  <button type="button" disabled>
                    当前账号
                  </button>
                ) : (
                  <button type="button" onClick={() => onSwitch(account)} disabled={isBusy}>
                    {isThisAccountBusy && busyAction?.kind === "switch" ? "正在切换…" : "切换账号"}
                  </button>
                )}
                <button type="button" onClick={() => onRelogin(account)} disabled={isBusy}>
                  {isThisAccountBusy && busyAction?.kind === "relogin" ? "登录中…" : "重新登录"}
                </button>
                <button
                  className="dangerButton"
                  type="button"
                  onClick={() => setPendingRemoval(account)}
                  disabled={isBusy}
                >
                  {isThisAccountBusy && busyAction?.kind === "remove" ? "退出中…" : "退出账号"}
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {pendingRemoval ? (
        <AccountRemovalDialog
          account={pendingRemoval}
          busy={isBusy}
          onCancel={() => setPendingRemoval(null)}
          onConfirm={confirmRemoval}
        />
      ) : null}
    </section>
  );
}
